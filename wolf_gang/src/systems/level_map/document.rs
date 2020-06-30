use crate::collections::octree;
use crate::level_map;

use legion::prelude::*;
use gdnative::*;
use bincode;

use serde::{Serialize, Deserialize};

type Octree = octree::Octree<i32, level_map::TileData>;

const MAP_PATH: &'static str = "user://maps/";

#[derive(Serialize, Deserialize)]
pub struct Document {
    file_name: Option<String>,
    title: String,
    data: Vec<Octree>
}

impl Document {
    pub fn new<S: ToString, T: ToString>(file_name: Option<S>, title: T) -> Self {

        let title = title.to_string();

        Document {
            file_name: match file_name {
                Some(r) => {
                    Some(r.to_string())
                },
                None => None
            },
            title,
            data: Vec::new()
        }
    }

    pub fn update_data(&mut self) {
        //go through and updata the data with the octree from each map chunk
        let map_query = <Read<level_map::MapChunkData>>::query();

        let mut game = crate::GAME_UNIVERSE.lock().unwrap();
        let game = &mut *game;
        let world = &mut game.world;

        let mut data: Vec<Octree> = Vec::new();
        for map_data in map_query.iter(world) {
            data.push(map_data.octree.clone());
        }

        self.data = data;
    }

    pub fn save(self) {

        match self.file_name.clone() {

            None => {
                panic!("Save was attempted on a document that doesn't have a file name");
            },

            Some(file_name) => {        

                let mut file = File::new();

                if let Ok(_) = file.open(GodotString::from(MAP_PATH.to_string() + &file_name), File::WRITE) {
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
    pub fn from_file<S: ToString>(file_name: S) -> Option<Self> {

        let file_name = file_name.to_string();

        let mut file = File::new();

        match file.open(GodotString::from(MAP_PATH.to_string() + &file_name), File::WRITE) {

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