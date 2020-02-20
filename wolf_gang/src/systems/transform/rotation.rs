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
    pub right: Vector3D,
    pub up: Vector3D,
    pub forward: Vector3D
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
    .with_query(<(Read<Rotation>, Write<Direction>, Tagged<node::NodeName>)>::query()
        .filter(changed::<Rotation>())
    )
    .build_thread_local(move |commands, world, resource, query| {

        for (rotation, mut direction, node_name) in query.iter(&mut *world) {
            let spatial_node : Option<Spatial> = {
                unsafe {
                    match node::find_node(GodotString::from_str(node_name.0.clone())) {
                        Some(r) => {
                            r.cast()
                        },
                        None => {
                            godot_print!("Can't find {:?}", node_name.0);                            

                            None
                        }
                    }
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