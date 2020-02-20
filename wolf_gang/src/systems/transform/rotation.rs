use gdnative::{godot_print, GodotString, Vector3, Spatial};

use legion::prelude::*;

type Vector3D = nalgebra::Vector3<f32>;
type Rotation3D = nalgebra::Rotation3<f32>;

use crate::node;

pub struct Rotation {
    pub value: Rotation3D
}

impl Default for Rotation {
    fn default() -> Self {
        Rotation {
            value: Rotation3D::new(Vector3D::new(0.,0.,0.))
        }
    }
}

pub struct Direction {
    right: Vector3D,
    up: Vector3D,
    forward: Vector3D
}

impl Default for Direction {
    fn default() -> Self {
        Direction {
            right: Vector3D::x(),
            up: Vector3D::y(),
            forward: Vector3D::z()
        }
    }
}

pub fn create_system_local() -> Box<dyn Runnable> {
    SystemBuilder::new("rotation_system")
    .with_query(<(Read<Rotation>, Write<Direction>, Read<node::NodeName>)>::query()
        .filter(changed::<Rotation>())
    )
    .build_thread_local(move |commands, world, resource, query| {

        for (rotation, mut direction, node_name) in query.iter(&mut *world) {
            let spatial_node : Option<Spatial> = match &node_name.name {
                Some(r) => {
                    unsafe {
                        match node::find_node(GodotString::from_str(r)) {
                            Some(r) => {
                                r.cast()
                            },
                            None => {
                                godot_print!("Can't find {:?}", r);                            

                                None
                            }
                        }
                    }
                },
                None => {
                    //some kind of error handling's gotta go here
                    godot_print!("Name is not set yet");
                    None
                }
            };

            match spatial_node {
                Some(mut r) => { 

                    direction.right = rotation.value * Vector3D::x();
                    direction.up = rotation.value * Vector3D::y();
                    direction.forward = rotation.value * Vector3D::z();

                    let euler = rotation.value.euler_angles();
                    unsafe { r.set_rotation(Vector3::new(euler.0, euler.1, euler.2)); } }
                None => {}
            }
        }
    })
}