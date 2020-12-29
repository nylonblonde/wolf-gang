use gdnative::godot_print;
use gdnative::api::{
    File
};

use serde::{Serialize, Deserialize};

type Point = nalgebra::Vector3<u32>;

#[derive(Clone, Serialize, Deserialize)]
pub struct ActorDefinitions {
    definitions: Vec<Actor>
}

impl ActorDefinitions {
    pub fn get_definitions(&self) -> &Vec<Actor> {
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

#[derive(Clone, Serialize, Deserialize)]
pub struct Actor {
    name: String,
    file_name: String,
    bounds: Point,
}

impl Actor {
    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_file_name(&self) -> &String {
        &self.file_name
    }

    pub fn get_bounds(&self) -> &Point {
        &self.bounds
    }
}