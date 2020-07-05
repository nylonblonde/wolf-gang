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

    pub fn save(self) {

        match self.file_path.clone() {

            None => {
                panic!("Save was attempted on a document that doesn't have a file name");
            },

            Some(file_path) => {     
                
                godot_print!("Saving {}", file_path);

                let mut file = File::new();

                if let Ok(_) = file.open(GodotString::from(file_path), File::WRITE) {
                    let encoded: Vec<u8> = bincode::serialize(&self).unwrap();

                    let mut byte_array = ByteArray::new();

                    for byte in encoded {
                        byte_array.push(byte);
                    }

                    file.store_buffer(byte_array);
                    file.close();
                }
            }
        }
    }

    /// Get document from file saved in user://maps/<file_name>
    pub fn from_file<S: ToString>(file_path: S) -> Option<Self> {

        let file_path = file_path.to_string();

        let mut file = File::new();

        match file.open(GodotString::from(file_path), File::WRITE) {

            Ok(_) => {
                
                let byte_array = file.get_buffer(file.get_len());

                let len = byte_array.len();

                let mut encoded: Vec<u8> = Vec::with_capacity(len as usize);

                for i in 0..len {
                    encoded.push(byte_array.get(i));
                }

                Some(bincode::deserialize::<Self>(&encoded).unwrap())
            },
            _err => {
                None
            }
        }
        
    }
}

impl Default for Document {
    fn default() -> Self {
        Document::new(Option::<String>::None, "Untitled")
    }
}