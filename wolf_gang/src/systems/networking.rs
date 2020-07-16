use legion::prelude::*;

use crate::{
    networking::{
        MessagePool, MessageSender
    }
};

/// This system takes messages and sends them to the message pool where they can be sent to the server
pub fn create_message_pooling_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("message_pooling_system")
        .write_resource::<MessagePool>()
        .with_query(<Read<MessageSender>>::query())
        .build(|commands, world, message_pool, query| {

            for (entity, message_sender) in query.iter_entities(world) {
                
                message_pool.messages.push((*message_sender).clone());
                commands.delete(entity);
            }
        })
}