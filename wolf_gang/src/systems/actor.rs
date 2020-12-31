use gdnative::prelude::*;
use gdnative::api::{
    File,
};
use legion::*;

use serde::{Serialize, Deserialize};

use crate::{
    node::{init_scene, NodeName},
    systems::{
        character_animator::{
            AnimationControlCreator,
            PlayAnimationState,
        },
        level_map,
        level_map::CoordPos,
        transform,
    },
};

type Point = nalgebra::Vector3<u32>;
type Vector3D = nalgebra::Vector3<f32>;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum ActorType {
    Player,
    Monument,
    Actor
}

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
pub struct ActorDefinitions {
    definitions: Vec<ActorDefinition>
}

impl ActorDefinitions {
    pub fn get_definitions(&self) -> &Vec<ActorDefinition> {
        &self.definitions
    }

    pub fn from_config() -> Option<ActorDefinitions> {
        {
            let path = "res://config/actors.ron";

            let file = File::new();

            if let true = file.file_exists(path) {
                godot_print!("File exists");
                if let Ok(_) = file.open(path, File::READ) {
        
                    let string = file.get_as_text().to_string();
        
                    match ron::de::from_str::<ActorDefinitions>(string.as_str()) {
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

pub trait Definition<'a>: Clone + Serialize + Deserialize<'a> {
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

impl Definition<'_> for CharacterDefinition {
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

impl Definition<'_> for ActorDefinition {
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