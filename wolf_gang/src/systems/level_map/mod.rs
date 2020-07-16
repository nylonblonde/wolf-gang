pub mod mesh;
pub mod history;
pub mod document;

use std::collections::HashSet;
use std::collections::HashMap;
use legion::prelude::*;
use serde::{Serialize, Deserialize};

use crate::{ 
    collections::{
        octree,
        octree::{ 
            Octree,
            PointData
        }
    },
    networking::{MessageSender, DataType}
};

#[cfg(not(test))]
use crate::systems::custom_mesh::MeshData;

type AABB = crate::geometry::aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;
type Vector3D = nalgebra::Vector3<f32>;


///ChangeType stores the range of the changes so that we can determine whether or not adjacent MapChunks actually need to change, and for Indirect changes,
/// the range of the original change for making comparisons
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ChangeType {
    Direct(AABB),
    Indirect(AABB)
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct ManuallyChange(ChangeType);

pub struct TileDimensions {
    pub x: f32,
    pub y: f32,
    pub z: f32
}

#[derive(Debug, Copy, Clone)]
pub struct CoordPos {
    pub value: Point
}

impl Default for CoordPos {
    fn default() -> CoordPos {
        CoordPos {
            value: Point::new(0,0,0)
        }
    }
}

pub const TILE_DIMENSIONS: TileDimensions = TileDimensions {x: 1.0, y: 0.25, z: 1.0};

/// Applies the const TILE_DIMENSIONS to each map coord to get its conversion in 3D space.
pub fn map_coords_to_world(map_coord: Point) -> nalgebra::Vector3<f32> {
    nalgebra::Vector3::<f32>::new(
        map_coord.x as f32 * TILE_DIMENSIONS.x, 
        map_coord.y as f32 * TILE_DIMENSIONS.y,
        map_coord.z as f32 * TILE_DIMENSIONS.z
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Message Type for sending and receiving changes to the map from the client. Changes are stored in an Octree, and mapchunks are updated accordingly.
pub struct MapInput {
    octree: Octree<i32, TileData>
}

impl MapInput {
    pub fn execute(&self, world: &mut legion::world::World, resources: &mut Resources) {

        let map = resources.get::<Map>().unwrap();

        let aabb = self.octree.get_aabb();

        let min = aabb.get_min();
        let max = aabb.get_max();

        let x_min_chunk = (min.x as f32 / map.chunk_dimensions.x as f32).floor() as i32;
        let y_min_chunk = (min.y as f32 / map.chunk_dimensions.y as f32).floor() as i32;
        let z_min_chunk = (min.z as f32 / map.chunk_dimensions.z as f32).floor() as i32;

        let x_max_chunk = (max.x as f32/ map.chunk_dimensions.x as f32).floor() as i32 + 1;
        let y_max_chunk = (max.y as f32/ map.chunk_dimensions.y as f32).floor() as i32 + 1;
        let z_max_chunk = (max.z as f32/ map.chunk_dimensions.z as f32).floor() as i32 + 1;

        let mut entities: HashMap<Entity, MapChunkData> = HashMap::new();
        let mut historically_significant: HashMap<Entity, MapChunkData> = HashMap::new();

        let min_chunk = Point::new(x_min_chunk, y_min_chunk, z_min_chunk);

        let dimensions = Point::new(x_max_chunk, y_max_chunk, z_max_chunk) - min_chunk;

        let volume = dimensions.x * dimensions.y * dimensions.z;
        
        for i in 0..volume {
            let x = x_min_chunk + i % dimensions.x;
            let y = y_min_chunk + (i / dimensions.x) % dimensions.y;
            let z = z_min_chunk + i / (dimensions.x * dimensions.y);

            let pt = Point::new(x,y,z);

            let map_chunk_exists_query = <Read<MapChunkData>>::query()
                .filter(tag_value(&pt));

            let mut exists = false;

            match map_chunk_exists_query.iter_entities(world).next() {
                Some((entity, map_chunk)) => {
                    println!("Map chunk exists already");
                    entities.insert(entity, map_chunk.as_ref().clone());
                    exists = true;
                },
                _ => {}
            }

            if !exists {
                println!("Creating a new map chunk at {:?}", pt);

                let (entity, map_data) = map.insert_mapchunk_with_octree(
                    &Octree::new(AABB::new(
                        Point::new(
                            pt.x * map.chunk_dimensions.x + map.chunk_dimensions.x/2,
                            pt.y * map.chunk_dimensions.y + map.chunk_dimensions.y/2,
                            pt.z * map.chunk_dimensions.z + map.chunk_dimensions.z/2,
                        ),
                        map.chunk_dimensions
                    ), octree::DEFAULT_MAX), 
                    world, false
                );

                entities.insert(entity, map_data);
            }
        }

        for (entity, map_data) in &mut entities {

            println!("Updating map_data");
            
            let map_aabb = map_data.octree.get_aabb();
            let overlap_aabb = aabb.get_intersection(map_aabb);

            let query_range = map_data.octree.query_range(overlap_aabb);

            let set = self.octree.clone().into_iter().collect::<HashSet<TileData>>();
            let map_set = query_range.into_iter().collect::<HashSet<TileData>>();

            if set.symmetric_difference(&map_set).count() == 0 {
                println!("Set and map_set were symmetrically the same");
                continue
            }

            //Remove any data that is in map_set but not set
            let difference = map_set.difference(&set);
            for item in difference {
                map_data.octree.remove_item(item);
            }

            //Add any data that is in set but not map_set
            let difference = set.difference(&map_set);
            for item in difference {
                map_data.octree.insert(*item).unwrap();
            }

            world.add_component(*entity, map_data.clone()).unwrap();
            world.add_tag(*entity, ManuallyChange(ChangeType::Direct(aabb))).unwrap();

            historically_significant.insert(*entity, map_data.clone());
        }

        let current_step = &mut *resources.get_mut::<crate::history::CurrentHistoricalStep>().unwrap();

        history::add_to_history(world, current_step, &mut historically_significant, CoordPos { value: aabb.center }, aabb);


        // for (entity, map_data) in &mut entities {

        //     let original = map_data.clone();

        //     let chunk_aabb = map_data.octree.get_aabb();
        //     let chunk_min = chunk_aabb.get_min();
        //     let chunk_max = chunk_aabb.get_max();

        //     let min_x = std::cmp::max(chunk_min.x, min.x);
        //     let min_y = std::cmp::max(chunk_min.y, min.y);
        //     let min_z = std::cmp::max(chunk_min.z, min.z);

        //     let max_x = std::cmp::min(chunk_max.x, max.x) + 1;
        //     let max_y = std::cmp::min(chunk_max.y, max.y) + 1;
        //     let max_z = std::cmp::min(chunk_max.z, max.z) + 1;

        //     let min = Point::new(min_x, min_y, min_z);
        //     let dimensions = Point::new(max_x, max_y, max_z) - min;
        //     let volume = dimensions.x * dimensions.y * dimensions.z;

        //     for i in 0..volume {
        //         let x = min_x + i % dimensions.x;
        //         let y = min_y + (i / dimensions.x) % dimensions.y;
        //         let z = min_z + i / (dimensions.x * dimensions.y);
            
        //         let pt = Point::new(x,y,z);

        //         match map_data.octree.insert(TileData{
        //             point: pt,
        //             ..tile_data
        //         }) {
        //             Ok(_) => {
        //             // println!("Inserted {:?}", pt);
        //             },
        //             Err(err) => {
        //                 println!("{:?}", err);
        //             }
        //         }
        //     }

        //     if map_data.octree.query_range(aabb).into_iter().collect::<HashSet<TileData>>().symmetric_difference(&original.octree.query_range(aabb).into_iter().collect::<HashSet<TileData>>()).count() > 0 {
        //         let map_data = map_data.clone();
                
        //         world.add_component(*entity, map_data.clone()).unwrap();
        //         world.add_tag(*entity, ManuallyChange(ChangeType::Direct(aabb))).unwrap();

        //         historically_significant.insert(*entity, map_data);
        //     }
                
        // }

        // history::add_to_history(world, current_step, &mut historically_significant, CoordPos { value: aabb.center }, aabb);

    }
}

#[derive(Copy, Clone)]
pub struct Map {
    chunk_dimensions: Point,
}

impl Default for Map {
    fn default() -> Self {
        Map { 
            chunk_dimensions: Point::new(10,10,10)
        }
    }
}

impl Map {

    /// Deletes all entities for the map chunks, removes the mesh nodes from the node cache, and resets the Document and CurrentHistoricalStep resources
    pub fn free(&self, world: &mut legion::world::World) {

        let map_chunk_query = <(Read<MapChunkData>, Tagged<crate::node::NodeName>)>::query();

        let mut entities: Vec<Entity> = Vec::new();

        for (entity, (_, node_name)) in map_chunk_query.iter_entities(world) {
            entities.push(entity);

            unsafe { crate::node::remove_node(node_name.0.clone()); }
        }

        for entity in entities {
            world.delete(entity);
        }
    }

    pub fn remove(&self, world: &mut legion::world::World, current_step: &mut crate::history::CurrentHistoricalStep, aabb: AABB) {
        let min = aabb.get_min();
        let max = aabb.get_max();

        let x_min_chunk = (min.x as f32 / self.chunk_dimensions.x as f32).floor() as i32;
        let y_min_chunk = (min.y as f32 / self.chunk_dimensions.y as f32).floor() as i32;
        let z_min_chunk = (min.z as f32 / self.chunk_dimensions.z as f32).floor() as i32;

        let x_max_chunk = (max.x as f32/ self.chunk_dimensions.x as f32).floor() as i32 + 1;
        let y_max_chunk = (max.y as f32/ self.chunk_dimensions.y as f32).floor() as i32 + 1;
        let z_max_chunk = (max.z as f32/ self.chunk_dimensions.z as f32).floor() as i32 + 1;

        let min_chunk = Point::new(x_min_chunk, y_min_chunk, z_min_chunk);

        let dimensions = Point::new(x_max_chunk, y_max_chunk, z_max_chunk) - min_chunk;

        let volume = dimensions.x * dimensions.y * dimensions.z;

        let mut to_update: HashMap<Entity, MapChunkData> = HashMap::new();
        let mut historically_significant: HashMap<Entity, MapChunkData> = HashMap::new();

        for i in 0..volume {
            let x = x_min_chunk + i % dimensions.x;
            let y = y_min_chunk + (i / dimensions.x) % dimensions.y;
            let z = z_min_chunk + i / (dimensions.x * dimensions.y);

            let pt = Point::new(x,y,z);

            let map_chunk_exists_query = <Write<MapChunkData>>::query()
                .filter(tag_value(&pt));

            if let Some((entity, map_data)) = map_chunk_exists_query.iter_entities_mut(world).next() {

                to_update.insert(entity, map_data.clone());
            }
        }

        for (entity, map_data) in &mut to_update {

            let original = map_data.clone();
            let mut map_data = map_data.clone();

            map_data.octree.remove_range(aabb);

            if map_data.octree.query_range(aabb).into_iter().collect::<HashSet<TileData>>().symmetric_difference(&original.octree.query_range(aabb).into_iter().collect::<HashSet<TileData>>()).count() > 0 {
                world.add_component(*entity, map_data.clone()).unwrap();
                world.add_tag(*entity, ManuallyChange(ChangeType::Direct(aabb))).unwrap();

                historically_significant.insert(*entity, map_data);
            }
        }

        history::add_to_history(world, current_step, &mut historically_significant, CoordPos { value: aabb.center }, aabb);

    }

    pub fn insert(&self, world: &mut legion::world::World, tile_data: TileData, aabb: AABB) {

        //Creare an octree from aabb with data from TileData
        let mut octree: Octree<i32, TileData> = Octree::new(aabb, octree::DEFAULT_MAX);

        let min = aabb.get_min();

        let dimensions = aabb.dimensions.abs();

        let volume = dimensions.x * dimensions.y * dimensions.z;

        for i in 0..volume {
            let x = min.x + i % dimensions.x;
            let y = min.y + (i / dimensions.x) % dimensions.y;
            let z = min.z + i / (dimensions.x * dimensions.y);

            octree.insert(TileData{
                point: Point::new(x,y,z),
                ..tile_data
            }).unwrap();
        }

        world.insert((), vec![
            (
                MessageSender{
                    data_type: DataType::MapInput(MapInput{
                        octree
                    }),
                    message_kind: cobalt::MessageKind::Ordered
                },
            )
        ]);

        // (client.0).0.send(Packet::reliable_ordered(
        //         (server.0).0.local_addr().unwrap(),         
        //         bincode::serialize(&MapInput{
        //             octree: Octree::new(aabb, octree::DEFAULT_MAX)
        //         }).unwrap(),
        //         None
        //     )
        // ).unwrap();

        // let message = udp::Message

        // let min = aabb.get_min();
        // let max = aabb.get_max();

        // let x_min_chunk = (min.x as f32 / self.chunk_dimensions.x as f32).floor() as i32;
        // let y_min_chunk = (min.y as f32 / self.chunk_dimensions.y as f32).floor() as i32;
        // let z_min_chunk = (min.z as f32 / self.chunk_dimensions.z as f32).floor() as i32;

        // let x_max_chunk = (max.x as f32/ self.chunk_dimensions.x as f32).floor() as i32 + 1;
        // let y_max_chunk = (max.y as f32/ self.chunk_dimensions.y as f32).floor() as i32 + 1;
        // let z_max_chunk = (max.z as f32/ self.chunk_dimensions.z as f32).floor() as i32 + 1;

        // let mut entities: HashMap<Entity, MapChunkData> = HashMap::new();
        // let mut historically_significant: HashMap<Entity, MapChunkData> = HashMap::new();

        // let min_chunk = Point::new(x_min_chunk, y_min_chunk, z_min_chunk);

        // let dimensions = Point::new(x_max_chunk, y_max_chunk, z_max_chunk) - min_chunk;

        // let volume = dimensions.x * dimensions.y * dimensions.z;

        // for i in 0..volume {
        //     let x = x_min_chunk + i % dimensions.x;
        //     let y = y_min_chunk + (i / dimensions.x) % dimensions.y;
        //     let z = z_min_chunk + i / (dimensions.x * dimensions.y);

        //     let pt = Point::new(x,y,z);

        //     let map_chunk_exists_query = <Read<MapChunkData>>::query()
        //         .filter(tag_value(&pt));

        //     let mut exists = false;

        //     match map_chunk_exists_query.iter_entities(world).next() {
        //         Some((entity, map_chunk)) => {
        //             println!("Map chunk exists already");
        //             entities.insert(entity, map_chunk.as_ref().clone());
        //             exists = true;
        //         },
        //         _ => {}
        //     }

        //     if !exists {
        //         println!("Creating a new map chunk at {:?}", pt);

        //         let (entity, map_data) = self.insert_mapchunk_with_octree(
        //             &Octree::new(AABB::new(
        //                 Point::new(
        //                     pt.x * self.chunk_dimensions.x + self.chunk_dimensions.x/2,
        //                     pt.y * self.chunk_dimensions.y + self.chunk_dimensions.y/2,
        //                     pt.z * self.chunk_dimensions.z + self.chunk_dimensions.z/2,
        //                 ),
        //                 self.chunk_dimensions
        //             ), octree::DEFAULT_MAX), 
        //             world, false
        //         );

        //         entities.insert(entity, map_data);
        //     }
        // }

        // for (entity, map_data) in &mut entities {

        //     let original = map_data.clone();

        //     let chunk_aabb = map_data.octree.get_aabb();
        //     let chunk_min = chunk_aabb.get_min();
        //     let chunk_max = chunk_aabb.get_max();

        //     let min_x = std::cmp::max(chunk_min.x, min.x);
        //     let min_y = std::cmp::max(chunk_min.y, min.y);
        //     let min_z = std::cmp::max(chunk_min.z, min.z);

        //     let max_x = std::cmp::min(chunk_max.x, max.x) + 1;
        //     let max_y = std::cmp::min(chunk_max.y, max.y) + 1;
        //     let max_z = std::cmp::min(chunk_max.z, max.z) + 1;

        //     let min = Point::new(min_x, min_y, min_z);
        //     let dimensions = Point::new(max_x, max_y, max_z) - min;
        //     let volume = dimensions.x * dimensions.y * dimensions.z;

        //     for i in 0..volume {
        //         let x = min_x + i % dimensions.x;
        //         let y = min_y + (i / dimensions.x) % dimensions.y;
        //         let z = min_z + i / (dimensions.x * dimensions.y);
            
        //         let pt = Point::new(x,y,z);

        //         match map_data.octree.insert(TileData{
        //             point: pt,
        //             ..tile_data
        //         }) {
        //             Ok(_) => {
        //             // println!("Inserted {:?}", pt);
        //             },
        //             Err(err) => {
        //                 println!("{:?}", err);
        //             }
        //         }
        //     }

        //     if map_data.octree.query_range(aabb).into_iter().collect::<HashSet<TileData>>().symmetric_difference(&original.octree.query_range(aabb).into_iter().collect::<HashSet<TileData>>()).count() > 0 {
        //         let map_data = map_data.clone();
                
        //         world.add_component(*entity, map_data.clone()).unwrap();
        //         world.add_tag(*entity, ManuallyChange(ChangeType::Direct(aabb))).unwrap();

        //         historically_significant.insert(*entity, map_data);
        //     }
                
        // }

        // history::add_to_history(world, current_step, &mut historically_significant, CoordPos { value: aabb.center }, aabb);

    }

    /// Inserts a new mapchunk with the octree data into world
    pub fn insert_mapchunk_with_octree(self, octree: &Octree<i32, TileData>, world: &mut World, changed: bool) -> (Entity, MapChunkData) {
        let map_data = MapChunkData{
            octree: octree.clone(),
        };

        let chunk_pt = map_data.get_chunk_point();

        let (entity, map_data) = (world.insert((chunk_pt,), vec![
            (
                map_data.clone(),
                history::MapChunkHistory::new(map_data.clone()),
                #[cfg(not(test))]
                MeshData::new(),
            )
        ])[0], map_data);

        if changed {
            world.add_tag(entity, ManuallyChange(ChangeType::Direct(octree.get_aabb()))).unwrap();
        }

        (entity, map_data)

    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapChunkData {
    octree: Octree<i32, TileData>,
}

impl MapChunkData {
    pub fn new(aabb: AABB) -> Self {
        MapChunkData {
            octree: Octree::new(aabb, octree::DEFAULT_MAX),
        }
    }

    pub fn get_chunk_point(&self) -> Point {
        let aabb = self.octree.get_aabb();
        let min = aabb.get_min();
        let dimensions = aabb.dimensions;

        Point::new(
            (min.x as f32 / dimensions.x as f32).floor() as i32,
            (min.y as f32 / dimensions.y as f32).floor() as i32,
            (min.z as f32 / dimensions.z as f32).floor() as i32,
        )
    }
}

#[derive(Serialize, Deserialize, Eq, Hash, PartialEq, Clone, Debug)]
pub struct TileData {
    point: Point
}

impl Copy for TileData {}

impl TileData {
    pub fn new(point: Point) -> Self {
        TileData {
            point
        }
    }
}

impl crate::collections::octree::PointData<i32> for TileData {

    fn get_point(&self) -> Point {
        self.point
    }
}