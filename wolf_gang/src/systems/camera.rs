use gdnative::*;

use legion::prelude::*;

use nalgebra;

use crate::node;
use crate::transform::{
    position::Position,
    rotation::Rotation
};

type Vector3D = nalgebra::Vector3<f32>;
type Rotation3D = nalgebra::Rotation3<f32>;

pub struct MainCamera {
    camera: Camera
}

pub struct FocalPoint {
    value: Vector3D
}

impl Default for FocalPoint {
    fn default() -> Self {
        FocalPoint { value: Vector3D::new(0.,0.,0.) }
    }
}

pub struct FocalAngle {
    euler: Vector3D
}

pub struct Zoom {
    value: f32
}

impl Default for Zoom {
    fn default() -> Self {
        Zoom { value: 10. }
    }
}

pub fn initialize_camera(world: &mut legion::world::World) -> Camera {
    
    let mut node_name = node::NodeName::new();
    
    let mut camera: Camera = Camera::new();

    unsafe { 
        node::add_node(&mut camera, Some(&mut node_name));

        camera.make_current();
    }

    world.insert((), vec![
        (
            node_name,
            Position::default(),
            FocalAngle {
                euler: Vector3D::new(-45., -45., 0.)
            },
            Rotation::default(),
            FocalPoint::default(),
            Zoom::default(),
        )
    ]);

    camera
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
            let euler = rot.euler_angles();

            rotation.euler = Vector3::new(euler.0, euler.1, euler.2);

        }
    })
}