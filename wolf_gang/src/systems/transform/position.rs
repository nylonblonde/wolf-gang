use gdnative::{godot_print, Vector3, Spatial};

use legion::prelude::*;

use crate::node;

pub struct Position {
    pub value: Vector3
}

impl Default for Position {
    fn default() -> Self {
        Position {
            value: Vector3::new(0.,0.,0.)
        }
    }
}

pub fn create_system_local() -> Box<dyn Runnable> {
    SystemBuilder::new("transform_position_system")
    .with_query(<(Read<Position>, Tagged<node::NodeName>)>::query()
        .filter(changed::<Position>())
    )
    .build_thread_local(move |commands, world, resource, query| {

        for (position, node_name) in query.iter(&mut *world) {
            // godot_print!("Move {:?}", node_name.name);

            let spatial_node : Option<Spatial> = {
                    unsafe {
                        match node::find_node(node_name.0.clone()) {
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
                    unsafe { r.set_translation(position.value); } }
                None => {}
            }
        
        }

    })
}