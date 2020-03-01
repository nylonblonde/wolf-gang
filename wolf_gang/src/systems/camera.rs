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
use crate::input::{ Action, InputActionComponent };
use crate::level_map;

type Vector3D = nalgebra::Vector3<f32>;
type Rotation3D = nalgebra::Rotation3<f32>;

pub struct FocalPoint(pub Vector3D);

pub struct FocalHeading(Vector3D);

impl Default for FocalPoint {
    fn default() -> Self {
        FocalPoint(Vector3D::zeros())
    }
}

impl Default for FocalHeading {
    fn default() -> Self {
        FocalHeading(Vector3D::zeros())
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
            FocalAngle(-45.0f32.to_radians(),225.0f32.to_radians(), 0.0),
            Rotation::default(),
            Direction::default(),
            FocalPoint::default(),
            FocalHeading::default(),
            Zoom::default(),
        )
    ]);

    node_name.0
}

pub fn create_movement_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("camera_movement_system")
    .read_resource::<crate::Time>()
    .with_query(<(Write<FocalPoint>, Read<FocalHeading>, Read<FocalAngle>, Read<Zoom>, Write<Position>)>::query()
        .filter(changed::<FocalPoint>() | changed::<Zoom>() | changed::<FocalAngle>())
    )
    .build(move |commands, world, time, query|{
        for (mut focal_point, focal_heading, focal_angle, zoom, mut position) in query.iter_mut(&mut *world) {

            focal_point.0 = focal_point.0 + (focal_heading.0 - focal_point.0) * time.delta * SPEED;

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

pub fn create_thread_local_fn() -> Box<dyn FnMut(&mut legion::world::World, &mut Resources)> {
    Box::new(|world: &mut legion::world::World, resources: &mut Resources| {
        let time = resources.get::<crate::Time>().unwrap();

        let camera_rotate_left = Action("camera_rotate_left".to_string());
        let camera_rotate_right = Action("camera_rotate_right".to_string());
        let camera_rotate_up = Action("camera_rotate_up".to_string());
        let camera_rotate_down = Action("camera_rotate_down".to_string());

        let input_query = <(Read<InputActionComponent>, Tagged<Action>)>::query()
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

        let selection_box_query = <(Read<selection_box::RelativeCamera>, Read<level_map::CoordPos>)>::query()
            .filter(changed::<crate::level_map::CoordPos>());

        unsafe{
            for (relative_cam, coord_pos) in selection_box_query.iter_unchecked(world) {
                let node_name = node::NodeName(relative_cam.0.clone());
                let cam_query = <Write<FocalHeading>>::query()
                    .filter(tag_value(&node_name));
                for mut focal_heading in cam_query.iter_unchecked(world) {
                    let center = level_map::map_coords_to_world(coord_pos.value);

                    let min = Vector3D::zeros();
                    let max = Vector3D::new(1.,1.,1.);

                    let mid = (max + min)/2.;

                    focal_heading.0 = center + mid;

                }
            }
        }
    })
}