use gdnative::godot_print;
use gdnative::api::{
    File
};

use serde::{Serialize, Deserialize};

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
    direction: Point,
}

pub struct Player {
    actor: Actor,
    character_definition: Option<CharacterDefinition>,
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