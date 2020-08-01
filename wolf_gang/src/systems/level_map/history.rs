//TODO: redo history based off of messages? That way you could undo/redo changes to the size of the selection_box. This current incarnation feels very janky and keeps being prone to weird edge cases
// Messages received should check that they came from this client so that only YOUR undo/redo history is saved

use legion::*;

use std::collections::VecDeque;
use super::{
    MapInput
};
use crate::{
    networking
};

pub struct MapInputHistory {
    history: VecDeque<MapInput>
}

impl MapInputHistory {
    pub fn new() -> Self {
        Self{
            history: VecDeque::new()
        }
    }
}

/// Takes map inputs, determines if they should be added to history (no duplicates), and creates a message if it should
pub fn create_map_input_system() -> impl systems::Schedulable {
    SystemBuilder::new("map_input_system")
        .with_query(<(Entity, Read<MapInput>)>::query())
        .build(|commands, world, _, query| {

            let mut map_messages: Vec<(networking::MessageSender,)> = Vec::new();
            query.for_each(world, |(entity, map_input)| {

                map_messages.push((networking::MessageSender{
                    data_type: networking::DataType::MapInput((*map_input).clone()),
                    message_type: networking::MessageType::Ordered
                },));

                commands.remove(*entity);
            });

            commands.extend(map_messages);
        })
}