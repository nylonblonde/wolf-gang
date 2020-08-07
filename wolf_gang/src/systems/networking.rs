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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Disconnection(u32);

impl Disconnection {
    pub fn new(id: u32) -> Self {
        Disconnection(id)
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

pub fn create_new_connection_thread_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {
    
    let mut query = <(Entity, Read<NewConnection>)>::query();
    
    Box::new(move |world, resources| {

        let results = query.iter(world)
            .map(|(entity, new_connection)| (*entity, *new_connection))
            .collect::<Vec<(Entity, NewConnection)>>();
        
        for (entity, new_connection) in results {
            crate::STATE_MACHINE.with(|s| {
                let state_machine = & *s.borrow();

                for state in &state_machine.states {
                    state.on_connection(new_connection.0, world, resources);
                }
            });

            //only need to act on a new connection once, get rid of the entity
            world.remove(entity);
        }
    })
}

pub fn create_disconnection_thread_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {
    
    let mut query = <(Entity, Read<Disconnection>)>::query();
    
    Box::new(move |world, resources| {

        let results = query.iter(world)
            .map(|(entity, disconnection)| (*entity, *disconnection))
            .collect::<Vec<(Entity, Disconnection)>>();
        
        for (entity, disconnection) in results {
            crate::STATE_MACHINE.with(|s| {
                let state_machine = & *s.borrow();

                for state in &state_machine.states {
                    state.on_disconnection(disconnection.0, world, resources);
                }
            });

            //only need to act on a disconnection once, get rid of the entity
            world.remove(entity);
        }
    })
}