use gdnative::prelude::*;
use gdnative::api::{
    Spatial,
};

use legion::*;

use crate::node;

#[derive(Copy, Clone)]
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

pub fn create_system() -> impl systems::Runnable {
    SystemBuilder::new("transform_position_system")
    .with_query(<(Read<Position>, Read<node::NodeName>)>::query()
        .filter(maybe_changed::<Position>())
    )
    .build(move |_, world, _, query| {

        query.for_each(world, |(position, node_name)| {
            // godot_print!("Move {:?}", node_name.name);

            let spatial_node : Option<Ref<Spatial>> = {
                    unsafe {
                        match node::get_node(&crate::OWNER_NODE.as_ref().unwrap().assume_safe(), &node_name.0, false) {
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

            if let Some(r) = spatial_node { 
                unsafe { r.assume_safe().set_translation(position.value); } 
            }
        
        })

    })
}