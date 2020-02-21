use gdnative::*;

use legion::prelude::*;

use nalgebra;

use crate::selection_box;
use crate::node;
use crate::transform;
use crate::transform::{
    position::Position,
    rotation::{ Rotation, Direction }
};
use crate::input::{ Action, InputComponent };
use crate::level_map;

type Vector3D = nalgebra::Vector3<f32>;
type Rotation3D = nalgebra::Rotation3<f32>;

pub struct FocalPoint{
    current: Vector3D,
    heading: Vector3D,
}

impl Default for FocalPoint {
    fn default() -> Self {
        FocalPoint {
            current: Vector3D::zeros(),
            heading: Vector3D::zeros()
        }
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
    
    let mut node_name = None;
    
    let mut camera: Camera = Camera::new();

    unsafe { 
        node_name = node::add_node(&mut camera);

        camera.make_current();
    }

    let node_name = node_name.unwrap();

    world.insert((node_name.clone(),), vec![
        (
            
            Position::default(),
            FocalAngle(-45.0f32.to_radians(),45.0f32.to_radians(), 0.0),
            Rotation::default(),
            Direction::default(),
            FocalPoint::default(),
            Zoom::default(),
        )
    ]);

    node_name.0
}

pub fn create_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("camera_system")
    // .read_resource::<crate::Time>()
    .with_query(<(Write<FocalPoint>, Read<FocalAngle>, Read<Zoom>, Write<Position>, Write<Rotation>)>::query()
        .filter(changed::<FocalPoint>() | changed::<Zoom>() | changed::<FocalAngle>())
    )
    .build(move |commands, world, time, query|{
        for (mut focal_point, focal_angle, zoom, mut position, mut rotation) in query.iter(&mut *world) {

            unsafe { focal_point.current = focal_point.current + (focal_point.heading - focal_point.current) * crate::DELTA_TIME as f32 * SPEED }

            // godot_print!("{:?} {:?}", focal_point.current, focal_point.heading);

            let new_position = focal_point.current + (Rotation3D::from_euler_angles(
                focal_angle.0, 
                focal_angle.1, 
                focal_angle.2
            ) * (Vector3D::z() * zoom.0));

            position.value = Vector3::new(new_position.x, new_position.y, new_position.z);

            let dir = new_position - focal_point.current;

            let up = Vector3D::y();

            let rot = Rotation3D::face_towards(&dir, &up);
            
            rotation.value = rot;

        }
    })
}

pub fn create_thread_local_fn() -> Box<dyn FnMut(&mut legion::world::World)> {
    Box::new(|world: &mut legion::world::World| {
        let camera_rotate_left = Action("camera_rotate_left".to_string());
        let camera_rotate_right = Action("camera_rotate_right".to_string());
        let camera_rotate_up = Action("camera_rotate_up".to_string());
        let camera_rotate_down = Action("camera_rotate_down".to_string());

        let input_query = <(Read<InputComponent>, Tagged<Action>)>::query()
            .filter(changed::<InputComponent>())
            .filter(
                tag_value(&camera_rotate_left)
                | tag_value(&camera_rotate_right)
                | tag_value(&camera_rotate_up)
                | tag_value(&camera_rotate_down)
            );
        
        unsafe {
            for(input_component, action) in input_query.iter_unchecked(world) {                    
                
                let cam_query = <Write<FocalAngle>>::query();
                for mut focal_angle in cam_query.iter_unchecked(world) {
                    if action.0 == camera_rotate_left.0 {
                        focal_angle.1 -= (input_component.strength * crate::DELTA_TIME) as f32 * SPEED;
                    } else if action.0 == camera_rotate_right.0 {
                        focal_angle.1 += (input_component.strength * crate::DELTA_TIME) as f32 * SPEED;
                    } else if action.0 == camera_rotate_up.0 {
                        focal_angle.0 -= (input_component.strength * crate::DELTA_TIME) as f32 * SPEED;
                    } else if action.0 == camera_rotate_down.0 {
                        focal_angle.0 += (input_component.strength * crate::DELTA_TIME) as f32 * SPEED;
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

        let selection_box_query = <(Read<selection_box::RelativeCamera>, Read<level_map::CoordPos>)>::query()
            .filter(changed::<crate::level_map::CoordPos>());

        unsafe{
            for (relative_cam, coord_pos) in selection_box_query.iter_unchecked(world) {
                let node_name = node::NodeName(relative_cam.0.clone());
                let cam_query = <Write<FocalPoint>>::query()
                    .filter(tag_value(&node_name));
                for mut focal_point in cam_query.iter_unchecked(world) {
                    let center = level_map::map_coords_to_world(coord_pos.value);

                    let min = Vector3D::zeros();
                    let max = Vector3D::new(1.,1.,1.);

                    let mid = (max + min)/2.;

                    focal_point.heading = center + mid;

                }
            }
        }
    })
}