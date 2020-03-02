use std::collections::HashMap;
use gdnative::*;

use legion::prelude::*;

use nalgebra;

use crate::selection_box;
use crate::node;
use crate::smoothing::Smoothing;
use crate::transform::{
    position::Position,
    rotation::{ Rotation, Direction }
};
use crate::input::{ Action, InputActionComponent };
use crate::level_map;

type Vector3D = nalgebra::Vector3<f32>;
type Rotation3D = nalgebra::Rotation3<f32>;

pub struct FocalPoint(pub Vector3D);

impl Default for FocalPoint {
    fn default() -> Self {
        FocalPoint(Vector3D::zeros())
    }
}

pub struct FocalAngle(pub f32, pub f32, pub f32);

pub struct Zoom(pub f32);

impl Default for Zoom {
    fn default() -> Self {
        Zoom(10.)
    }
}

const SPEED : f32 = 4.;

pub fn initialize_camera(world: &mut legion::world::World) -> String {
    
    
    let mut camera: Camera = Camera::new();

    let node_name = unsafe { 
        node::add_node(&mut camera)
    };

    unsafe {
        camera.make_current();
    }

    let node_name = node_name.unwrap();

    world.insert((node_name.clone(),), vec![
        (
            
            Position::default(),
            FocalAngle(-45.0f32.to_radians(),225.0f32.to_radians(), 0.0),
            Rotation::default(),
            Direction::default(),
            FocalPoint::default(),
            Zoom::default(),
        )
    ]);

    node_name.0
}

pub fn create_movement_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("camera_movement_system")
    .with_query(<(Read<FocalPoint>, Read<FocalAngle>, Read<Zoom>, Write<Position>)>::query()
        .filter(changed::<FocalPoint>() | changed::<Zoom>() | changed::<FocalAngle>())
    )
    .build(move |_, world, _, query|{
        for (focal_point, focal_angle, zoom, mut position) in query.iter_mut(&mut *world) {

            let new_position = focal_point.0 + (Rotation3D::from_euler_angles(
                focal_angle.0, 
                focal_angle.1, 
                focal_angle.2
            ) * (Vector3D::z() * zoom.0));

            position.value = Vector3::new(new_position.x, new_position.y, new_position.z);
        }
    })
}

pub fn create_rotation_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("camera_rotation_system")
    .with_query(<(Read<FocalPoint>, Read<Position>, Write<Rotation>)>::query()
        .filter(changed::<Position>())
    )
    .build(move |_, world, _, query|{
        for (focal_point, position, mut rotation) in query.iter_mut(&mut *world) {

            let dir = Vector3D::new(position.value.x, position.value.y, position.value.z) - focal_point.0;

            let up = Vector3D::y();

            let rot = Rotation3D::face_towards(&dir, &up);
            
            rotation.value = rot;

        }
    })
}

/// Handles the input for rotating the camera around the focal point
pub fn create_camera_angle_thread_local_fn() -> Box<dyn FnMut(&mut legion::world::World, &mut Resources)> {
    let camera_rotate_left = Action("camera_rotate_left".to_string());
    let camera_rotate_right = Action("camera_rotate_right".to_string());
    let camera_rotate_up = Action("camera_rotate_up".to_string());
    let camera_rotate_down = Action("camera_rotate_down".to_string());

    let cam_query = <Write<FocalAngle>>::query();

    Box::new(move |world: &mut legion::world::World, resources: &mut Resources| {
        let time = resources.get::<crate::Time>().unwrap();

        let input_query = <(Read<InputActionComponent>, Tagged<Action>)>::query()
        .filter(
            tag_value(&camera_rotate_left)
            | tag_value(&camera_rotate_right)
            | tag_value(&camera_rotate_up)
            | tag_value(&camera_rotate_down)
        );

        unsafe {
            for(input_component, action) in input_query.iter_unchecked(world) {                    
                
                for mut focal_angle in cam_query.iter_unchecked(world) {
                    if action.0 == camera_rotate_left.0 {
                        focal_angle.1 -= input_component.strength as f32 * time.delta * SPEED;
                    } else if action.0 == camera_rotate_right.0 {
                        focal_angle.1 += input_component.strength as f32 * time.delta * SPEED;
                    } else if action.0 == camera_rotate_up.0 {
                        focal_angle.0 -= input_component.strength as f32 * time.delta * SPEED;
                    } else if action.0 == camera_rotate_down.0 {
                        focal_angle.0 += input_component.strength as f32 * time.delta * SPEED;
                    }

                    let min = -(std::f32::consts::FRAC_PI_2 - 0.001);
                    if focal_angle.0 < min {
                        focal_angle.0 = min
                    } else if focal_angle.0 > 0. {
                        focal_angle.0 = 0.
                    }
                }
            }
        } 
    })
}

///Updates the focal point of the camera when a smoothing entity is present
pub fn create_focal_point_thread_local_fn() -> Box<dyn FnMut(&mut legion::world::World, &mut Resources)> {

    Box::new(move |world: &mut legion::world::World, _| {

        let selection_box_query_relative_cam = <Read<selection_box::RelativeCamera>>::query();

        unsafe{
            for relative_cam in selection_box_query_relative_cam.iter_unchecked(world) {
                let node_name = node::NodeName(relative_cam.0.clone());
                let smoothing_query = <Write<Smoothing>>::query()
                        .filter(tag_value(&node_name));

                match smoothing_query.iter_unchecked(world).next() {
                    Some(smoothing) => {
                
                        let cam_query = <Write<FocalPoint>>::query()
                            .filter(tag_value(&node_name));

                        for mut focal_point in cam_query.iter_unchecked(world) {
                            focal_point.0 = smoothing.current;
                        }
                    },
                    None => {
                    }
                    
                }
            }
        }
    })
}


/// Creates a smoothing entity that will handle smoothing between the selection box's position and the current focal point
pub fn create_follow_selection_box_thread_local_fn() -> Box<dyn FnMut(&mut legion::world::World, &mut Resources)> {
    let selection_box_query = <(Read<selection_box::RelativeCamera>, Read<level_map::CoordPos>)>::query()
        .filter(changed::<crate::level_map::CoordPos>());

    Box::new(move |world: &mut legion::world::World, _| {

        let mut entities_to_insert: HashMap<String, Smoothing> = HashMap::new();
        unsafe{
            for (relative_cam, coord_pos) in selection_box_query.iter_unchecked(world) {
                let node_name = node::NodeName(relative_cam.0.clone());

                let cam_query = <Read<FocalPoint>>::query()
                    .filter(tag_value(&node_name));
                    
                for focal_point in cam_query.iter_unchecked(world) {
                    let center = level_map::map_coords_to_world(coord_pos.value);

                    let min = Vector3D::zeros();
                    let max = Vector3D::new(1.,1.,1.);

                    let mid = (max + min)/2.;

                    let heading = center + mid;

                    let smoothing_query = <Write<Smoothing>>::query()
                        .filter(tag_value(&node_name));

                    match smoothing_query.iter_unchecked(world).next() {
                        Some(mut r) => {
                            r.heading = heading;
                        },
                        None => {
                            entities_to_insert.insert(node_name.0.clone(),
                                Smoothing{
                                    current: focal_point.0,
                                    heading,
                                    speed: SPEED
                                }
                            );
                        }
                    }
                }
            }
        }

        for (name, smoothing) in entities_to_insert {
            world.insert((node::NodeName(name),), vec![(smoothing,)]);
        }
    })
}