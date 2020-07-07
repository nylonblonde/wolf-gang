use crate::collections::octree;
use crate::systems::level_map;

use legion::prelude::*;
use gdnative::*;
use bincode;

use serde::{Serialize, Deserialize};

type Octree = octree::Octree<i32, level_map::TileData>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub file_path: Option<String>,
    pub title: String,
    data: Vec<Octree>
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
            data: Vec::new()
        }
    }

    ///Updates the data for the document by iterating through queries on the world    
    pub fn update_data(&mut self, world: &mut legion::world::World) {
        //go through and updata the data with the octree from each map chunk
        let map_query = <Read<level_map::MapChunkData>>::query();

        let mut data: Vec<Octree> = Vec::new();
        for map_data in map_query.iter(world) {
            data.push(map_data.octree.clone());
        }

        self.data = data;
    }

    /// Populate the world with the required entities
    pub fn populate_world(&self, world: &mut legion::world::World, resources: &mut Resources) {

        match resources.get::<level_map::Map>() {
            Some(map) => {
                
                for octree in &self.data {

                    godot_print!("inserting map chunk");

                    map.insert_mapchunk_with_octree(octree.clone(), world, true);
                }

            },
            None => panic!("Couldn't find the Map resource when populating the world")
        }

    }

    /// Returns a Vec<u8> of the result of serializing the document using bincode
    pub fn to_raw(&self) -> Vec<u8> {
        let encoded: Vec<u8> = bincode::serialize(self).unwrap();

        encoded
    }

    /// Helper function to get a ByteArray for use in Godot's buffer and file classes
    pub fn to_byte_array(original: Vec<u8>) -> ByteArray {
        let mut byte_array = ByteArray::new();

        for byte in original {
            byte_array.push(byte);
        }

        byte_array
    }

    pub fn save(self) {

        match self.file_path.clone() {

            None => {
                panic!("Save was attempted on a document that doesn't have a file name");
            },

            Some(file_path) => {     
                
                godot_print!("Saving {}", file_path);

                let mut file = File::new();

                if let Ok(_) = file.open(GodotString::from(file_path), File::WRITE) {
                    let encoded = self.to_raw();

                    let byte_array = Self::to_byte_array(encoded);

                    file.store_buffer(byte_array);
                    file.close();
                }
            }
        }
    }

    pub fn raw_from_file<S: ToString>(file_path: S) -> Vec<u8> {
        let file_path = file_path.to_string();

        let mut file = File::new();

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

        let result = bincode::deserialize::<Self>(&raw);

        for octree in &result.as_ref().unwrap().data {
            godot_print!("{:?}", octree.query_range(octree.get_aabb()));
        }

        result
        
    }

    pub fn from_raw(raw: &Vec<u8>) -> Result<Self, Box<bincode::ErrorKind>> {
        bincode::deserialize::<Self>(raw)
    }
}

impl Default for Document {
    fn default() -> Self {
        Document::new(Option::<String>::None, "Untitled")
    }
}