use std::collections::VecDeque;

use legion::*;

use serde::{Serialize, Deserialize};

use crate::{
    collections::octree::Octree,
    systems::{ 
        input::{
            InputActionComponent, Action
        },
        level_map::{Map, TileData,},
        networking::{ 
            ClientID, DataType, MessageSender, MessageType
        },
    },
    Time
};

use std::io::{ Error, ErrorKind };

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StepType {
    MapChange((Octree<i32, TileData>, Octree<i32, TileData>)),
    // "previous" state must be an option to capture the state where the actor did not exist
    // ActorChange((ActorChange, ActorChange)),
}

/// Resource which holds chnages as a VecDeque
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct History {
    history: VecDeque<StepType>,
    current_step: i32,
    previous_amount: i32,
}

impl History {
    pub fn new() -> Self {
        History {
            history: VecDeque::new(),
            current_step: -1,
            previous_amount: -1,
        }
    }

    pub fn add_step(&mut self, step: StepType) {

        //if there is a history beyond this step, wipe it out
        if self.current_step > -1 && self.history.len() as i32 > self.current_step + 1 {
            //this will always be shrinking so the generator is unreachable - there's nothing to generate
            self.history.resize_with(self.current_step as usize, || unreachable!());
        }

        self.history.push_back(step);
        self.current_step = self.history.len() as i32;
        self.previous_amount = -1;

        println!("Current step is {}", self.current_step);
    }

    /// Moves forward or backward in history by the given amount
    pub fn move_by_step(&mut self, commands: &mut legion::systems::CommandBuffer, resources: &mut Resources, amount: i32) {

        if let Ok((step, next_step)) = self.determine_move(amount) {
            match step {
                StepType::MapChange((undo_map, redo_map)) => {
                    if let Some(map) = resources.get::<Map>().map(|map| *map) {
                        let octree = if amount > 0 { redo_map.clone() } else { undo_map.clone() };

                        commands.exec_mut(move |world, _| {
                            map.change(world, octree.clone(), None);
                        })
                    }
                },
                // StepType::ActorChange((undo_actor, redo_actor)) => {
                //     if let Some(actor_definitions) = resources.get::<ActorDefinitions>() {
                        
                //         let change = if amount > 0 { *redo_actor } else { *undo_actor };
            
                //         let actor_definitions = actor_definitions.clone();
                        
                //         commands.exec_mut(move |world| {
                //             actor_change(world, &change, &actor_definitions, None);
                //         })
                //     }
                    
                // },
            }

            self.current_step = std::cmp::max(0, std::cmp::min(self.history.len() as i32 - 1, next_step));
            self.previous_amount = amount;
        }
    }

    fn determine_move(&'_ self, amount: i32) -> Result<(&'_ StepType, i32), Error> {
        let mut next_step = self.current_step as i32 + amount;

        //since current_step was determined by the previous step, make an adjustment if we've actually changed direction in the history this time
        if num::signum(amount) != num::signum(self.previous_amount) {
            next_step -= amount;
        }

        //Just so I don't try and clean this up again, we do the seemingly redundant range checks because next_step can be -1 and will cause a panic if converted to usize
        if next_step > -1 && next_step < self.history.len() as i32 {
            Ok(
                (&self.history[next_step as usize], next_step)
            )
        } else {
            Err(Error::new(ErrorKind::NotFound, ""))
        }
    }

    /// If there are steps further back than the current step
    pub fn can_undo(&'_ self) -> Result<&'_ StepType, Error>  {
        self.determine_move(-1).map(|(x, _)| x)
    }

    /// If there are steps ahead of the current step
    pub fn can_redo(&'_ self) -> Result<&'_ StepType, Error> {
        self.determine_move(1).map(|(x, _)| x)
    }
}

pub fn send_move_by_step(commands: &mut legion::systems::CommandBuffer, client_id: u32, amount: i32) {
    commands.push(
        (
            MessageSender{
                data_type: DataType::HistoryStep{
                    client_id,
                    amount
                },
                message_type: MessageType::Ordered
            },
        )
    );
}

pub fn create_history_input_system() -> impl systems::Runnable {

    let undo = Action("undo".to_string());
    let redo = Action("redo".to_string());

    SystemBuilder::new("history_input_system")
        .read_resource::<ClientID>()
        .read_resource::<Time>()
        .with_query(<(Read<InputActionComponent>, Read<Action>)>::query())
        .build(move |commands, world, (client_id, time), query| {

            for (input_component, action) in query.iter(world).filter(|(_,a)|
                *a == &undo ||
                *a == &redo
            ) {
                if input_component.repeated(time.delta, 0.25) {
                    if action == &undo {
                        send_move_by_step(commands, client_id.val(), -1);
                    } else if action == &redo {
                        send_move_by_step(commands, client_id.val(), 1);
                    }
                }
            }
        })
}
