use gdnative::{godot_print, GodotString, Node, Vector3, Spatial};

use legion::prelude::*;

use crate::node;

pub struct Position {
    pub value: Vector3
}

pub fn create_system_local() -> Box<dyn Runnable> {
    SystemBuilder::new("transform_position_system")
    .with_query(<(Read<Position>, Read<node::NodeName>)>::query()
        .filter(changed::<Position>())
    )
    .build_thread_local(move |commands, world, resource, query| {

        for (position, node_name) in query.iter(&mut *world) {
            // godot_print!("Move {:?}", node_name.name);

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
                    unsafe { r.set_translation(position.value); } }
                None => {}
            }
        
        }

    })
}