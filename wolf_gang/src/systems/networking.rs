use legion::*;

use serde::{Serialize, Deserialize};

use crate::{
    networking::{
        MessagePool, MessageSender
    },
};


#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct NewConnection(u32);

impl NewConnection {
    pub fn new(id: u32) -> Self {
        NewConnection(id)
    }
}

/// This system takes messages and sends them to the message pool where they can be sent to the server
pub fn create_message_pooling_system() -> impl systems::Schedulable {
    SystemBuilder::new("message_pooling_system")
        .write_resource::<MessagePool>()
        .with_query(<(Entity, Read<MessageSender>)>::query())
        .build(|commands, world, message_pool, query| {

            for (entity, message_sender) in query.iter(world) {
                
                message_pool.messages.push((*message_sender).clone());
                commands.remove(*entity);
            }
        })
}

/// This system calls on on_connection for the game states
pub fn create_new_connection_system() -> impl systems::Schedulable {

    SystemBuilder::new("new_connection_system")
        .with_query(<(Entity, Read<NewConnection>)>::query())
        .build(|commands, world, _, query| {

            for (entity, new_connection) in query.iter(world) {
                
                let entity = *entity;
                let connection_id = new_connection.0;

                commands.exec_mut(move |world| {

                    crate::STATE_MACHINE.with(|s| {
                        let state_machine = & *s.borrow();

                        for state in &state_machine.states {

                            state.on_connection(connection_id, world);

                            println!("Called new_connection on {:?}", state.as_ref().as_ref().get_name());
                        }
                    });

                    //only need to act on a new connection once, get rid of the entity
                    world.remove(entity);
                });
                
            }
        })
}