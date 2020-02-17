use gdnative::{godot_print, GodotString, Vector3, Spatial};

use legion::prelude::*;

use crate::node;

pub struct Rotation {
    pub euler: Vector3
}

impl Default for Rotation {
    fn default() -> Self {
        Rotation {
            euler: Vector3::new(0.,0.,0.)
        }
    }
}

pub fn create_system_local() -> Box<dyn Runnable> {
    SystemBuilder::new("rotation_system")
    .with_query(<(Read<Rotation>, Read<node::NodeName>)>::query()
        .filter(changed::<Rotation>())
    )
    .build_thread_local(move |commands, world, resource, query| {

        for (rotation, node_name) in query.iter(&mut *world) {
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
                    unsafe { r.set_rotation(rotation.euler); } }
                None => {}
            }
        }
    })
}