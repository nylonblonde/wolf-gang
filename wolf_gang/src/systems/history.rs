use std::collections::VecDeque;

use legion::*;

use crate::{
    collections::octree::Octree,
    systems::{ 
        input::{
            InputActionComponent, Action
        },
        level_map::TileData,
        networking::{ 
            ClientID, DataType, MessageSender, MessageType
        },
    },
    Time
};

pub enum StepTypes {
    MapChange(Octree<i32, TileData>)
}

/// This component is basically used as a flag to keep track of whether or not a change came from the history, thus blocking the change from being written to history when handled by its system.
pub struct IsFromHistory{}

/// Resource which holds chnages as a VecDeque
pub struct History {
    history: VecDeque<StepTypes>,
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

    pub fn add_step(&mut self, step: StepTypes) {

        //if there is a history beyond this step, wipe it out
        if self.current_step > -1 && self.history.len() as i32 > self.current_step {
            //this will always be shrinking so the generator is unreachable - there's nothing to generate
            self.history.resize_with(self.current_step as usize, || unreachable!());
        }

        self.history.push_back(step);
        self.current_step = self.history.len() as i32;

        println!("Current step is {}", self.current_step);
    }

    /// Moves forward or backward in history by the given amount
    pub fn move_by_step(&mut self, buffer: &mut systems::CommandBuffer, amount: i32) {

        // let mut next_step = self.current_step as i32 + amount;

        // //since current_step was determined by the previous step, make an adjustment if we've actually changed direction in the history this time
        // if num::signum(amount) != num::signum(self.previous_amount) {
        //     next_step -= amount;
        // }

        // let step: Option<&StepTypes> = if next_step > -1 && next_step < self.history.len() as i32 {
        //     Some(&self.history[next_step as usize])
        // } else if next_step < 0 {
        //     Some(&self.history[0])
        // } else if next_step > self.history.len() as i32 -1 {
        //     Some(&self.history[self.history.len()-1])
        // } else {
        //     None
        // };

        // if let Some(step) = step {
        //     match step {
        //         StepTypes::MapChange((undo_map, redo_map)) => {
        //             let map_input = if amount > 0 { redo_map } else { undo_map };

        //             buffer.push(
        //                 (
        //                     (*map_input).clone(),
        //                     IsFromHistory{}
        //                 )
        //             );
        //         }
        //     }
        // }

        // self.current_step = std::cmp::max(-1, std::cmp::min(self.history.len() as i32, next_step));
        // self.previous_amount = amount;

    }

    /// If there are steps further back than the current step
    pub fn can_undo (&self) -> bool {

        //todo: need a better way of checking in undos can be done - since the oldest undo is technically at index 0
        self.current_step > 0 && self.history.len() > 0
    }

    /// If there are steps ahead of the current step
    pub fn can_redo(&self) -> bool {
        self.history.len() > 0 && self.current_step < self.history.len() as i32 - 1
    }
}

pub fn send_move_by_step(commands: &mut legion::systems::CommandBuffer, client_id: u32, amount: i32) {
    commands.push(
        (
            MessageSender{
                data_type: DataType::HistoryStep{
                    client_id: client_id,
                    amount: 1
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
