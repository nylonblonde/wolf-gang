use gdnative::prelude::*;
use gdnative::api::{
    File,
};
use legion::*;

use serde::{Serialize, Deserialize};

use crate::{
    node,
    node::{init_scene, NodeName},
    systems::{
        character_animator::{
            AnimationControlCreator,
            PlayAnimationState,
        },
        history::{
            History, StepType
        },
        level_map,
        level_map::CoordPos,
        networking::ClientID,
        transform,
    },
};

type Point = nalgebra::Vector3<i32>;
type Vector3D = nalgebra::Vector3<f32>;
type AABB = crate::geometry::aabb::AABB<i32>;

/// Message data type for communicating changes over the connection
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum ActorChange {
    ActorInsertion{
        /// unique id for the inserted actor object
        uuid: u128,
        /// the index of the actor in ActorDefinitions
        definition_id: u32,
        coord_pos: Point,
        rotation: nalgebra::Rotation3<f32>,
        // rh side is sub_definition
        actor_type: (ActorType, Option<u32>),
    },
    ActorRemoval(u128),
}

// #[derive(Debug, Copy, Clone)]
// pub enum ActorChangeOption {
//     Some(ActorChange),
//     None(u128)
// }

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum ActorType {
    Player,
    Monument,
    Actor
}

#[derive(Copy, Clone, PartialEq)]
pub struct ActorID(u128);

impl ActorID {
    pub fn val(&self) -> u128 {
        self.0
    }
}

#[derive(Clone)]
pub struct Actor {
    definition_id: usize,
    definition: ActorDefinition,
}

impl Actor {

    pub fn new(actor_definitions: &Definitions<ActorDefinition>, definition_id: usize) -> Actor {
        
        let definition = actor_definitions.get_definitions().get(definition_id).unwrap().clone();
        
        Actor {
            definition_id,
            definition
        }
    }

    pub fn get_definition(&self) -> &ActorDefinition{
        &self.definition
    }

    pub fn get_bounds(&self, rotation: nalgebra::Rotation3<f32>) -> AABB {

        let bounds = self.definition.bounds;

        let bounds = Point::new(
            (bounds.x as f32 / level_map::TILE_DIMENSIONS.x) as i32,
            (bounds.y as f32 / level_map::TILE_DIMENSIONS.y) as i32,
            (bounds.z as f32 / level_map::TILE_DIMENSIONS.z) as i32,
        );
        
        let aabb = AABB::new(Point::zeros(), bounds);
        return aabb.rotate(rotation)
    }
}

pub struct Player {
    actor: Actor,
    character_definition: Option<CharacterDefinition>,
}

impl Player {
    pub fn get_actor_definition(&self) -> &ActorDefinition{
        &self.actor.get_definition()
    }
    pub fn get_character_definition(&self) -> Option<&CharacterDefinition> {
        self.character_definition.as_ref()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Definitions<T> {
    definitions: Vec<T>,
}

pub trait DefinitionsTrait<T: Sized>: serde::de::DeserializeOwned {
    fn get_definitions(&self) -> &Vec<T>;

    fn from_config(path: &'static str) -> Option<Self> where Self: Sized{
        {
            // let path = "res://config/actors.ron";

            let file = File::new();

            if let true = file.file_exists(path) {
                godot_print!("File exists");
                if let Ok(_) = file.open(path, File::READ) {
        
                    let string = file.get_as_text().to_string();
        
                    match ron::de::from_str::<Self>(string.as_str()) {
                        Ok(result) => Some(result),
                        Err(err) => {
                            println!("{:?}", err);
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}

impl DefinitionsTrait<ActorDefinition> for Definitions<ActorDefinition> {
    fn get_definitions(&self) -> &Vec<ActorDefinition> {
        &self.definitions
    }
}

pub trait Definition: serde::de::DeserializeOwned + Clone + Serialize {
    fn get_name(&self) -> &String;
    fn get_path(&self) -> &String;
    fn get_bounds(&self) -> &Vector3D;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CharacterDefinition {
    name: String,
    path: String,
    bounds: Vector3D,

    //this is a seperate struct because we could possibly fit other properties here, like path to voice sounds, prefab outfits, other configurables
}

impl Definition for CharacterDefinition {
    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_path(&self) -> &String {
        &self.path
    }

    fn get_bounds(&self) -> &Vector3D {
        &self.bounds
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ActorDefinition {
    name: String,
    path: String,
    bounds: Vector3D,
    actor_type: ActorType,
}

impl ActorDefinition {
    pub fn get_type(&self) -> &ActorType {
        &self.actor_type
    }
}

impl Definition for ActorDefinition {
    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_path(&self) -> &String {
        &self.path
    }

    fn get_bounds(&self) -> &Vector3D {
        &self.bounds
    }
}

pub fn place_actor(world: &mut World, actor_entity: Entity)  {

    let bounds = if let Some(entry) = world.entry(actor_entity) {
        if let Ok(actor) = entry.get_component::<Actor>() {
            if let Ok(coord_pos) = entry.get_component::<CoordPos>(){
                if let Ok(rotation) = entry.get_component::<transform::rotation::Rotation>() {
                    let bounds = AABB::new(coord_pos.value, actor.get_bounds(rotation.value).dimensions);

                    Some(bounds)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some(bounds) = bounds {
        position_actor_helper(world, actor_entity, bounds);   
    }
}

pub fn position_actor_helper(world: &mut World, actor_entity: Entity, aabb: AABB) {
    if let Ok(entry) = world.entry_mut(actor_entity) {
        godot_print!("what");
        if let Ok(position) = entry.into_component_mut::<transform::position::Position>() {

            let min = level_map::map_coords_to_world(aabb.get_min());

            godot_print!("{:#?}", min);

            let bounds = level_map::map_coords_to_world(aabb.dimensions);
            let bounds = Vector3::new(bounds.x, bounds.y, bounds.z);
            position.value = Vector3::new(min.x, min.y, min.z) + Vector3::new(bounds.x/2., 0., bounds.z/2.);
        }
    } 
}

pub fn actor_change(
    world: &mut World, 
    change: &ActorChange, 
    actor_definitions: &Definitions<ActorDefinition>, 
    character_definitions: Option<&Definitions<CharacterDefinition>>, 
    store_history: Option<u32>
) {

    match change {
        ActorChange::ActorInsertion {
            uuid, 
            coord_pos, 
            rotation, 
            actor_type, 
            definition_id, 
        } => match actor_type {
            (ActorType::Actor, _) => {
                let mut query = <(Read<ActorID>, Read<Actor>, Read<CoordPos>, Read<transform::rotation::Rotation>)>::query();

                let results = query.iter(world)
                    .filter(|(actor_id, _,_,_)| actor_id.val() == *uuid)
                    .map(|(actor_id, actor, coord_pos, rotation)| (*actor_id, actor.clone(), *coord_pos, *rotation))
                    .collect::<Vec::<(ActorID, Actor, CoordPos, transform::rotation::Rotation)>>();

                if results.len() > 0 { //If an actor with this uuid already exists
                    results.into_iter().for_each(|(actor_id, actor, coord_pos, rotation)| {

                        //TODO: update values for existing actor

                    });
                } else { // if an actor with this uuid does not exist

                    // add to history
                    //TODO: Check that there is actually changes worth storing
                    if let Some(client_id) = store_history {
                        let mut query = <(Write<History>, Read<ClientID>)>::query();    

                        if let Some((history, _)) = query.iter_mut(world).filter(|(_, id)| id.val() == client_id).next() {
                            history.add_step(StepType::ActorChange(
                                (ActorChange::ActorRemoval(*uuid), change.clone())
                            ))
                        }
                    }

                    // this block creates a new actor
                    let owner = unsafe { crate::OWNER_NODE.as_ref().unwrap() };
                    let entity = initialize_actor(world, unsafe{ &owner.assume_safe()}, &Actor::new(actor_definitions, *definition_id as usize));
        
                    if let Some(mut entry) = world.entry(entity) {
                        entry.add_component(CoordPos::new(*coord_pos));
                        entry.add_component(transform::rotation::Rotation{value:*rotation});
                        entry.add_component(ActorID(*uuid));
                    }

                    place_actor(world, entity);
                }
                
            },
            _ => {
                unimplemented!();
            }
        },
        ActorChange::ActorRemoval(uuid) => remove_actor(world, *uuid)
    }
}

pub fn remove_actor(world: &mut World, uuid: u128) {

    let mut query = <(Read<NodeName>, Read<ActorID>)>::query();

    let results = query.iter(world)
        .filter(|(_, actor_id)| actor_id.val() == uuid)
        .map(|(node_name, _)| node_name.clone())
        .collect::<Vec<NodeName>>();

    results.into_iter().for_each(|node_name| {
        node::free(world, &node_name.0);
    });
}

pub fn initialize_actor(world: &mut World, parent: &Node, actor: &Actor) -> Entity {

    let node = unsafe { init_scene(world, parent, actor.definition.get_path().to_string()).assume_safe() };

    let node_name = NodeName(node.name().to_string());

    world.push(
        (
            node_name,
            actor.clone(),
            AnimationControlCreator{},
            PlayAnimationState("square_up".to_string()),
            transform::position::Position::default(),
            transform::rotation::Rotation::default(),
        )
    )
}