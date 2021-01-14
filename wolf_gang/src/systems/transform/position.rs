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
    .with_query(<(Read<Position>, Read<node::NodeRef>)>::query()
        .filter(maybe_changed::<Position>())
    )
    .build(move |_, world, _, query| {

        query.for_each(world, |(position, node_ref)| {

            let spatial_node = unsafe { node_ref.val().assume_safe().cast::<Spatial>().unwrap().as_ref().assume_shared() };

            unsafe { spatial_node.assume_safe().set_translation(position.value); } 
        
        })

    })
}