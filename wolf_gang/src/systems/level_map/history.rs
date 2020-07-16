//TODO: redo history based off of messages? That way you could undo/redo changes to the size of the selection_box. This current incarnation feels very janky and keeps being prone to weird edge cases

use std::collections::HashMap;
use legion::prelude::*;
use crate::{
    history,
    systems::{selection_box, input, level_map}
};

use super::MapChunkData;

use gdnative::godot_print;

type AABB = crate::geometry::aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;

#[derive(Clone)]
pub struct MapChunkHistory {
    pub steps: Vec<MapChunkChange>
}

impl MapChunkHistory {
    pub fn new(first_data: level_map::MapChunkData) -> MapChunkHistory {
        MapChunkHistory {
            steps: vec![MapChunkChange{
                step_changed_at: crate::history::CurrentHistoricalStep(0),
                coord_pos: level_map::CoordPos::default(),
                aabb: AABB::new(Point::zeros(), Point::new(1,1,1)),
                map_chunk_data: first_data
            }]
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
    pub aabb: AABB,
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

pub fn add_to_history(world: &mut World, current_step: &mut crate::history::CurrentHistoricalStep, entities: &mut HashMap<Entity, MapChunkData>, coord_pos: super::CoordPos, aabb: AABB) {
    let history_query = <Write<MapChunkHistory>>::query();

    let mut to_update: HashMap<Entity, MapChunkHistory> = HashMap::with_capacity(entities.len());

    for (entity, mut map_history) in history_query.iter_entities_mut(world) {

        //truncate all steps that are ahead of this one (to remove the ability to redo if a new action takes place after undos have taken place)
        let mut truncate_at: Option<usize> = None;
        let len = map_history.steps.len();
        for i in 0..len {

            let step = &map_history.steps[i];

            if step.step_changed_at.0 >= current_step.0 {
                truncate_at = Some(i);
                break;
            }
        }

        if let Some(index) = truncate_at {
            map_history.steps.truncate(index);
        }

        match entities.get_mut(&entity) {

            Some(map_chunk_data) => {

                map_history.steps.push(MapChunkChange{
                    coord_pos,
                    aabb,
                    step_changed_at: current_step.clone(),
                    map_chunk_data: map_chunk_data.clone()
                });

                to_update.insert(entity, map_history.clone());

            },

            None => {}
        }
    }

    for (entity, map_history) in to_update.clone() {

        godot_print!("latest history changed at {}", map_history.steps.last().unwrap().step_changed_at.0);

        world.add_component(entity, map_history).unwrap();
        
    }

    if to_update.len() > 0 {
        current_step.0 += 1;
    }

}

/// Move to a historical step by the amount, and update CurrentHistoricalStep with where we are
pub fn move_to_step(world: &mut World, current_step: &mut history::CurrentHistoricalStep, amount: i32) {

    godot_print!("current = {}", current_step.0);

    let next_step = current_step.0 as i32 + amount - 1;

    if next_step < 0 {
        return
    }

    let target_step = history::CurrentHistoricalStep(next_step as u32);

    let map_query = <(Write<MapChunkData>, Read<MapChunkHistory>)>::query();

    let mut entities: HashMap<Entity, (MapChunkChange, AABB)> = HashMap::new();

    for (entity, (mut map_chunk, map_history)) in map_query.iter_entities_mut(world) {

        godot_print!("map_history_len = {}", map_history.steps.len());

        let len = map_history.steps.len();

        for i in 0..len {

            let steps = &map_history.clone().steps;

            let change = &steps[i];

            //this can only get the most recent aabb of the current chunk if the chunk if at the beginning of its history.
            let aabb = if amount < 0 && i + 1 < len {
                steps[(i as i32 + 1) as usize].aabb
            } else {
                change.aabb
            };

            if change.step_changed_at == target_step {
                *map_chunk = change.map_chunk_data.clone();
                
                entities.insert(entity, (change.clone(), aabb));
                break;

            } else if change.step_changed_at.0 > target_step.0 { //if the next change is past the target step, move to the previous in the list

                let previous_chunk = map_history.steps[i-1].map_chunk_data.clone();
                
                if previous_chunk != *map_chunk {
                    *map_chunk = previous_chunk;
                    entities.insert(entity, (change.clone(), aabb));
                }
                break;
            }
        }
    }

    if let Some((mut selection_box, mut coord_pos)) = <(Write<selection_box::SelectionBox>, Write<super::CoordPos>)>::query().iter_mut(world).next() {
        if let Some((_, (change, _))) = entities.clone().into_iter().next() {
            selection_box.aabb.dimensions = change.aabb.dimensions;
            *coord_pos = change.coord_pos;
        }
    }

    if entities.len() > 0 {
        *current_step = history::CurrentHistoricalStep(target_step.0 as u32 + 1);
    }

    for (entity, (_, aabb)) in entities {
        world.add_tag(entity, super::ManuallyChange(super::ChangeType::Direct(aabb))).unwrap();
    }

}