use legion::prelude::*;
use crate::{history, input};

use super::MapChunkData;

use gdnative::godot_print;

#[derive(Clone)]
pub struct MapChunkHistory {
    pub steps: Vec<MapChunkChange>
}

impl MapChunkHistory {
    pub fn new() -> MapChunkHistory {
        MapChunkHistory {
            steps: Vec::new()
        }
    }
}

#[derive(Clone)]
/// MapChunkChange represents any changes that get stored in the history of the chunk.
pub struct MapChunkChange {
    /// When in history the change occurred (for checking against the global historical step)
    pub step_changed_at: crate::history::CurrentHistoricalStep,
    /// Stores the CoordPos so that we can move the cursor back to the spot it was in for that step
    pub coord_pos: super::CoordPos,
    pub map_chunk_data: MapChunkData
}

pub fn create_undo_redo_input_system() -> Box<dyn FnMut(&mut World, &mut Resources)> {

    Box::new(|world: &mut World, resources: &mut Resources| {

        let historical_step = match resources.get::<history::CurrentHistoricalStep>() {
            Some(r) => r,
            None => {
                panic!("No resource for CurrentHistoricalStep exists in the world");
            }
        };

        let undo_action = input::Action("undo".to_string());
        let redo_action = input::Action("redo".to_string());

        let input_query = <(Read<input::InputActionComponent>, Tagged<input::Action>)>::query()
            .filter(tag_value(&undo_action) | tag_value(&redo_action));

        for (input_component, action) in input_query.iter(world) {

            if input_component.just_pressed() {

                if action.0 == undo_action.0 {
                    undo(*historical_step);
                } else if action.0 == redo_action.0 {
                    redo(*historical_step);
                }
            }

        }

    })

}

fn undo(step: history::CurrentHistoricalStep) {
    godot_print!("Undo");
}

fn redo(step: history::CurrentHistoricalStep) {
    godot_print!("Redo");
}