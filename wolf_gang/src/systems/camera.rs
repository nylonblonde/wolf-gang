use gdnative::*;

use legion::prelude::*;

use nalgebra;

use crate::node;
use crate::transform::{
    position::Position,
    rotation::{ Rotation, Direction }
};
use crate::input::{ Action, InputComponent };

type Vector3D = nalgebra::Vector3<f32>;
type Rotation3D = nalgebra::Rotation3<f32>;

pub struct FocalPoint {
    pub value: Vector3D
}

impl Default for FocalPoint {
    fn default() -> Self {
        FocalPoint { value: Vector3D::new(0.,0.,0.) }
    }
}

pub struct FocalAngle {
    pub euler: Vector3D
}

pub struct Zoom {
    pub value: f32
}

impl Default for Zoom {
    fn default() -> Self {
        Zoom { value: 10. }
    }
}

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
            FocalAngle {
                euler: Vector3D::new(-45., 360.-45., 0.)
            },
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
    .with_query(<(Read<FocalPoint>, Read<FocalAngle>, Read<Zoom>, Write<Position>, Write<Rotation>)>::query()
        .filter(changed::<FocalPoint>() | changed::<Zoom>() | changed::<FocalAngle>())
    )
    .build(move |commands, world, resources, query|{
        for (focal_point, focal_angle, zoom, mut position, mut rotation) in query.iter(&mut *world) {
            let new_position = focal_point.value + (Rotation3D::from_euler_angles(focal_angle.euler.x.to_radians(), focal_angle.euler.y.to_radians(), 0.) * (Vector3D::z() * zoom.value));

            position.value = Vector3::new(new_position.x, new_position.y, new_position.z);

            let dir = (new_position - focal_point.value).normalize();

            let up = Vector3D::y();
            let rot = Rotation3D::face_towards(&dir, &up);

            rotation.value = rot;

        }
    })
}

pub fn create_thread_local_fn() -> Box<dyn FnMut(&mut legion::world::World)> {
    Box::new(|world: &mut legion::world::World| {
        let camera_rotate_left = Action("camera_rotate_left".to_string());
        let camera_rotate_right = Action("camera_rotate_left".to_string());
        let camera_rotate_up = Action("camera_rotate_left".to_string());
        let camera_rotate_down = Action("camera_rotate_left".to_string());

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

            }
        }
    })
}