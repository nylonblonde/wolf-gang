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

/// Message data type for communicating changes over the connection
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum ActorChange {
    ActorInsertion{
        /// unique id for the inserted actor object
        uuid: u128,
        /// the index of the actor in ActorDefinitions
        definition_id: u32,
        coord_pos: Point,
        direction: Vector3D,
        actor_type: ActorType,
        sub_definition: Option<u32>,
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
    definiton: ActorDefinition,
}

impl Actor {
    pub fn get_definition(&self) -> &ActorDefinition{
        &self.definiton
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

pub struct MoveImmediate{}

pub fn create_move_actor_system() -> impl systems::Runnable {
    SystemBuilder::new("move_actor_system")
        .with_query(<(Entity, Read<NodeName>)>::query()
            .filter(component::<MoveImmediate>())
        )
        .with_query(<(Read<NodeName>, Read<Actor>, Read<CoordPos>, Write<transform::position::Position>)>::query())
        .build(move |commands,world,_,queries| {
            let (move_queries, actor_queries) = queries;

            let results = move_queries.iter(world)
                .map(|(entity, node_name)| (*entity, node_name.clone()))
                .collect::<Vec<(Entity, NodeName)>>();
            
            results.into_iter().for_each(move |(entity, node_name)| {
                
                actor_queries.iter_mut(world).filter(|(name, _, _, _)| *name == &node_name)
                    .for_each(|(_, actor, coord_pos, position)| {
                        let mut new_position = level_map::map_coords_to_world(coord_pos.value);
                        let bounds = actor.definiton.get_bounds();
                        new_position += Vector3D::new(bounds.x, -bounds.y, bounds.z) / 2.;
                        position.value = Vector3::new(new_position.x, new_position.y, new_position.z);
                    });

                commands.exec_mut(move |world| {
                    world.remove(entity);
                });

            });
        })
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
            direction, 
            actor_type, 
            definition_id, 
            sub_definition 
        } => match actor_type {
            ActorType::Actor => {
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
                    if let Some(actor_definition) = actor_definitions.get_definitions().get(*definition_id as usize) {
                        let entity = initialize_actor(world, actor_definition, CoordPos::new(*coord_pos));
            
                        if let Some(mut entry) = world.entry(entity) {
                            entry.add_component(ActorID(*uuid));
                        }
                    }
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

pub fn initialize_actor(world: &mut World, actor_definition: &ActorDefinition, coord_pos: CoordPos) -> Entity {

    let owner = unsafe { crate::OWNER_NODE.as_mut().unwrap().assume_safe() };
    let node = unsafe { init_scene(world, &owner, actor_definition.get_path().to_string()).assume_safe() };

    let node_name = NodeName(node.name().to_string());

    world.push(
        (
            MoveImmediate{},
            node_name.clone(),
        )
    );

    world.push(
        (
            node_name,
            Actor{
                definiton: actor_definition.clone(),    
            },
            coord_pos,
            AnimationControlCreator{},
            PlayAnimationState("square_up".to_string()),
            transform::position::Position::default(),
            transform::rotation::Rotation::default(),
        )
    )
}