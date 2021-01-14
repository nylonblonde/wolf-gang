use crate::{
    collections::octree,
    systems::{
        actor,
        level_map,
        networking::{
            DataType, MessageSender, MessageType
        },
    },
};

use legion::*;
use gdnative::prelude::*;
use gdnative::api::{
    File,
};

use serde::{Serialize, Deserialize};

use std::collections::HashSet;

type Octree = octree::Octree<i32, level_map::TileData>;

pub struct ResetMap{}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub file_path: Option<String>,
    pub title: String,
    map_chunks: Vec<Octree>,
    actor_data: Option<Vec<u8>>,
}

impl Document {
    pub fn new<S: ToString, T: ToString>(file_path: Option<S>, title: T) -> Self {

        let title = title.to_string();

        Document {
            file_path: match file_path {
                Some(r) => {
                    Some(r.to_string())
                },
                None => None
            },
            title,
            map_chunks: Vec::new(),
            actor_data: None,
        }
    }

    ///Updates the data for the document by iterating through queries on the world    
    pub fn update_data(&mut self, world: &mut legion::world::World) {
        //go through and updata the data with the octree from each map chunk
        let mut map_query = <Read<level_map::MapChunkData>>::query();

        let mut data: Vec<Octree> = Vec::new();
        for map_data in map_query.iter(world) {
            data.push(map_data.octree.clone());
        }

        self.map_chunks = data;

        //get actor data
        if let Ok(serialized) = actor::serialize_actors_in_world(world) {
            self.actor_data = Some(serialized);
        }
    }

    /// Populate the world with the required entities from self's document data
    pub fn populate_world(&self, world: &mut legion::world::World, _resources: &mut Resources) {

        for octree in &self.map_chunks {
            world.push(
                (
                    MessageSender{
                        data_type: DataType::MapInput(octree.clone()),
                        message_type: MessageType::Ordered
                    },
                )
            );
        }

        if let Some(actor_data) = &self.actor_data {
            world.push(
                (
                    MessageSender{
                        data_type: DataType::ActorChange {
                            change: actor::ActorChange::ActorInsertion {
                                serialized: actor_data.to_vec()
                            },
                            store_history: None,
                        },
                        message_type: MessageType::Ordered,
                    },
                )
            );
        }
    }

    /// Returns a Vec<u8> of the result of serializing the document using bincode
    pub fn to_raw(&self) -> Vec<u8> {
        let encoded: Vec<u8> = bincode::serialize(self).unwrap();

        encoded
    }

    pub fn save(&self) {

        match self.file_path.clone() {

            None => {
                panic!("Save was attempted on a document that doesn't have a file name");
            },

            Some(file_path) => {     
                
                godot_print!("Saving {}", file_path);

                let file = File::new();

                if file.open(GodotString::from(file_path), File::WRITE).is_ok() {
                    let encoded = self.to_raw();

                    let byte_array = vec_to_byte_array(encoded);

                    file.store_buffer(byte_array);
                    file.close();
                }
            }
        }
    }

    /// Returns true if there are unsaved changes to this file, including if a saved file doesn't exist. Remember to call update_data before calling
    pub fn has_unsaved_changes(&self) -> bool {
        match &self.file_path {
            Some(file_path) => {

                let working_file = Document {
                    file_path: self.file_path.clone(),
                    title: self.title.clone(),
                    ..Default::default()
                };

                match Document::from_file(file_path) {
                    Ok(opened_file) => {
                        let opened_file = Document {
                            file_path: opened_file.file_path,
                            title: opened_file.title,
                            ..Default::default()
                        };

                        let mut working_data = HashSet::new();

                        working_file.map_chunks.iter().for_each(|octree| {
                            working_data.extend(octree.clone().into_iter())
                        });

                        let mut opened_data = HashSet::new();

                        opened_file.map_chunks.iter().for_each(|octree| {
                            opened_data.extend(octree.clone().into_iter())
                        });

                        opened_file == working_file && opened_data.symmetric_difference(&working_data).count() == 0
                    },
                    _ => self != &Document::default()
                }
        
            },
            None => self != &Document::default()
        }
    }

    pub fn raw_from_file<S: ToString>(file_path: S) -> Vec<u8> {
        let file_path = file_path.to_string();

        let file = File::new();

        match file.open(GodotString::from(file_path), File::READ) {

            Ok(_) => {
                
                let byte_array = file.get_buffer(file.get_len());

                let len = byte_array.len();

                let mut encoded: Vec<u8> = Vec::with_capacity(len as usize);

                for i in 0..len {
                    encoded.push(byte_array.get(i));
                }

                encoded

            },
            err => panic!("{:?}", err)
            
        }
    }

    pub fn from_file<S: ToString>(file_path: S) -> Result<Self, Box<bincode::ErrorKind>> {

        let raw = Self::raw_from_file(file_path);

        bincode::deserialize::<Self>(&raw)
        
    }

    pub fn from_raw(raw: &[u8]) -> Result<Self, Box<bincode::ErrorKind>> {
        bincode::deserialize::<Self>(raw)
    }
}

/// Helper function to get a ByteArray for use in Godot's buffer and file classes
pub fn vec_to_byte_array(original: Vec<u8>) -> ByteArray {
    let mut byte_array = ByteArray::new();

    for byte in original {
        byte_array.push(byte);
    }

    byte_array
}

impl Default for Document {
    fn default() -> Self {
        Document::new(Option::<String>::None, "Untitled")
    }
}