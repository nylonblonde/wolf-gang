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

#[derive(Debug, Clone)]
/// MapChunkChange represents any changes that get stored in the history of the chunk.
pub struct MapChunkChange {
    /// When in history the change occurred (for checking against the global historical step)
    pub step_changed_at: crate::history::CurrentHistoricalStep,
    /// Stores the CoordPos so that we can move the cursor back to the spot it was in for that step
    pub coord_pos: super::CoordPos,
    pub map_chunk_data: MapChunkData
}

enum UndoRedo {
    None, Undo, Redo
}

pub fn create_undo_redo_input_system() -> Box<dyn FnMut(&mut World, &mut Resources)> {

    Box::new(|world: &mut World, resources: &mut Resources| {

        let mut undo_redo = UndoRedo::None;

        let mut historical_step = match resources.get_mut::<history::CurrentHistoricalStep>() {
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
                    undo_redo = UndoRedo::Undo;
                } else if action.0 == redo_action.0 {
                    undo_redo = UndoRedo::Redo;
                }
            }
        }

        match undo_redo {
            UndoRedo::Undo => {
                move_to_step(world, &mut historical_step, -1);
            },
            UndoRedo::Redo => {
                move_to_step(world, &mut historical_step, 1);
            },
            UndoRedo::None => {}
        }

    })

}

/// Move to a historical step by the amount, and update CurrentHistoricalStep with where we are
fn move_to_step(world: &mut World, current_step: &mut history::CurrentHistoricalStep, amount: i32) {

    //FIXME: Redo doesn't go all the way back to the most recent history

    let next_step = current_step.0 as i32 + amount;

    if next_step < 0 {
        return
    }

    godot_print!("{}", next_step);

    let target_step = history::CurrentHistoricalStep(next_step as u32);

    let map_query = <(Write<MapChunkData>, Read<MapChunkHistory>)>::query();

    let mut entities: Vec<Entity> = Vec::new();

    for (entity, (mut map_chunk, map_history)) in map_query.iter_entities_mut(world) {

        godot_print!("Did the query even find anything");

        for change in map_history.steps.clone() {

            godot_print!("change: {:?}", change);

            if change.step_changed_at == target_step {

                godot_print!("Found the target step");

                *map_chunk = change.map_chunk_data;

                entities.push(entity);

            }

        }

    }

    for entity in entities {
        world.add_tag(entity, super::ManuallyChange(super::ChangeType::Direct)).unwrap();
    }

    *current_step = target_step;

}