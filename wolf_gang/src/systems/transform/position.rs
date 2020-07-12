use gdnative::prelude::*;
use gdnative::api::{
    Spatial,
};

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
    .build_thread_local(move |_, world, _, query| {

        for (position, node_name) in query.iter(&mut *world) {
            // godot_print!("Move {:?}", node_name.name);

            let spatial_node : Option<Ref<Spatial>> = {
                    unsafe {
                        match node::get_node(&crate::OWNER_NODE.as_ref().unwrap().assume_safe(), node_name.0.clone()) {
                            Some(r) => {
                                Some(r.assume_safe().cast::<Spatial>().unwrap().as_ref().assume_shared())
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
                    unsafe { r.assume_safe().set_translation(position.value); } }
                None => {}
            }
        
        }

    })
}