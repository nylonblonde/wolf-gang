use gdnative::prelude::*;
use gdnative::api::{
    Spatial,
};

use legion::*;

use crate::node;

use serde::{Serialize, Deserialize};

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Position {
    pub value: nalgebra::Vector3<f32>
}

impl Default for Position {
    fn default() -> Self {
        Position {
            value: nalgebra::Vector3::new(0.,0.,0.)
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

            let position = position.value;

            unsafe { spatial_node.assume_safe().set_translation(Vector3::new(position.x, position.y, position.z)); } 
        
        })

    })
}