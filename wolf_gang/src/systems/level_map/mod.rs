pub mod mesh;
pub mod document;

use std::sync::mpsc;
use std::collections::HashSet;
use std::collections::HashMap;
use legion::*;
use serde::{Serialize, Deserialize};
use rayon::prelude::*;

use crate::{ 
    collections::{
        octree,
        octree::{ 
            Octree,
        }
    },
    systems::{
        custom_mesh,
        history::{History, StepTypes},
        networking
    },
    node::{NodeName}
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
pub struct MapInput {
    octree: Octree<i32, TileData>
}

impl MapInput {
    pub fn get_octree(&self) -> &Octree<i32, TileData> {
        &self.octree
    }
//     pub fn execute(self, world: &mut legion::world::World, resources: &mut Resources) {
//         change_map(world, resources, self.octree);
//     }
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

    pub fn change(&self, world: &mut legion::world::World, octree: &Octree<i32, TileData>) {

        let aabb = octree.get_aabb();
    
        let min = aabb.get_min();
        let max = aabb.get_max();
    
        let x_min_chunk = (min.x as f32 / self.chunk_dimensions.x as f32).floor() as i32;
        let y_min_chunk = (min.y as f32 / self.chunk_dimensions.y as f32).floor() as i32;
        let z_min_chunk = (min.z as f32 / self.chunk_dimensions.z as f32).floor() as i32;
    
        let x_max_chunk = (max.x as f32/ self.chunk_dimensions.x as f32).floor() as i32 + 1;
        let y_max_chunk = (max.y as f32/ self.chunk_dimensions.y as f32).floor() as i32 + 1;
        let z_max_chunk = (max.z as f32/ self.chunk_dimensions.z as f32).floor() as i32 + 1;
    
        let mut entities: HashMap<Entity, MapChunkData> = HashMap::new();
        // let mut historically_significant: HashMap<Entity, MapChunkData> = HashMap::new();
    
        let min_chunk = Point::new(x_min_chunk, y_min_chunk, z_min_chunk);
    
        let dimensions = Point::new(x_max_chunk, y_max_chunk, z_max_chunk) - min_chunk;
    
        let volume = dimensions.x * dimensions.y * dimensions.z;
        
        for i in 0..volume {
            let x = x_min_chunk + i % dimensions.x;
            let y = y_min_chunk + (i / dimensions.x) % dimensions.y;
            let z = z_min_chunk + i / (dimensions.x * dimensions.y);
    
            let pt = Point::new(x,y,z);
    
            let mut map_chunk_exists_query = <(Entity, Read<MapChunkData>, Read<Point>)>::query();
    
            let mut exists = false;
    
            match map_chunk_exists_query.iter(world).filter(|(_, _, chunk_pt)| **chunk_pt == pt).next() {
                Some((entity, map_chunk, _)) => {
                    println!("Map chunk exists already");
                    entities.insert(*entity, map_chunk.clone());
                    exists = true;
                },
                _ => {}
            }
    
            if !exists {
                println!("Creating a new map chunk at {:?}", pt);
    
                let (entity, map_data) = self.insert_mapchunk_with_octree(
                    &Octree::new(AABB::new(
                        Point::new(
                            pt.x * self.chunk_dimensions.x + self.chunk_dimensions.x/2,
                            pt.y * self.chunk_dimensions.y + self.chunk_dimensions.y/2,
                            pt.z * self.chunk_dimensions.z + self.chunk_dimensions.z/2,
                        ),
                        self.chunk_dimensions
                    ), octree::DEFAULT_MAX), 
                    world, false
                );
    
                entities.insert(entity, map_data);
            }
        }
    
        for (entity, map_data) in &mut entities {
            
            let map_aabb = map_data.octree.get_aabb();
            let overlap_aabb = aabb.get_intersection(map_aabb);
    
            let map_query_range = map_data.octree.query_range(overlap_aabb);
            let input_query_range = octree.query_range(overlap_aabb);
    
            let set = input_query_range.into_iter().collect::<HashSet<TileData>>();
            let map_set = map_query_range.into_iter().collect::<HashSet<TileData>>();
    
            // if set.symmetric_difference(&map_set).count() == 0 {
            //     println!("Set and map_set were symmetrically the same");
            //     continue
            // }
    
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
    
            if let Some(mut entry) = world.entry(*entity) {
                entry.add_component(map_data.clone());
                entry.add_component(ManuallyChange(ChangeType::Direct(aabb)));
            }
        }
    }

    /// Deletes all entities for the map chunks, removes the mesh nodes from the node cache
    pub fn free(&self, world: &mut legion::world::World) {

        let mut map_chunk_query = <(Entity, Read<NodeName>)>::query()
            .filter(component::<MapChunkData>());

        let results = map_chunk_query.iter(world)
            .map(|(entity, node_name)| (*entity, (*node_name).clone()))
            .collect::<Vec<(Entity, NodeName)>>();

        for (entity, node_name) in results {
            unsafe { crate::node::remove_node(&node_name.0); }
            world.remove(entity);
        }
    }

    pub fn remove(&self, world: &mut legion::world::World, aabb: AABB) {

        world.push(
            (
                MapInput {
                    octree: Octree::new(aabb, octree::DEFAULT_MAX)
                },
            )
        );
    }

    pub fn insert(&self, world: &mut legion::world::World, tile_data: TileData, aabb: AABB) {

        let mut octree = Octree::new(aabb, octree::DEFAULT_MAX);

        let min = aabb.get_min();

        let dimensions = aabb.dimensions.abs();

        let volume = dimensions.x * dimensions.y * dimensions.z;

        let (tx, rx) = mpsc::channel::<TileData>();

        (0..volume).into_par_iter().for_each_with(tx, move |tx, i| {
            let x = min.x + i % dimensions.x;
            let y = min.y + (i / dimensions.x) % dimensions.y;
            let z = min.z + i / (dimensions.x * dimensions.y);

            tx.send(TileData {
                point: Point::new(x,y,z),
                ..tile_data
            }).unwrap();
        });

        rx.into_iter().for_each(|x| {
            octree.insert(x).unwrap()
        });

        // create an entity for the change, which has a system which will check if it is not identical to the most recent historical step, and forward it along as a 
        // MessageSender if it is appropriate to do so
        world.push(
            (
                MapInput {
                    octree
                },
            )
        );
    }

    /// Does a query range on every chunk that fits within the range
    fn query_chunk_range(&self, map_datas: Vec<(&MapChunkData, &Point)>, range: AABB) -> Vec<TileData> {
        let min = range.get_min();
        let max = range.get_max();

        let x_min_chunk = (min.x as f32 / self.chunk_dimensions.x as f32).floor() as i32;
        let y_min_chunk = (min.y as f32 / self.chunk_dimensions.y as f32).floor() as i32;
        let z_min_chunk = (min.z as f32 / self.chunk_dimensions.z as f32).floor() as i32;

        let x_max_chunk = (max.x as f32/ self.chunk_dimensions.x as f32).floor() as i32 + 1;
        let y_max_chunk = (max.y as f32/ self.chunk_dimensions.y as f32).floor() as i32 + 1;
        let z_max_chunk = (max.z as f32/ self.chunk_dimensions.z as f32).floor() as i32 + 1;

        let min_chunk = Point::new(x_min_chunk, y_min_chunk, z_min_chunk);

        let dimensions = Point::new(x_max_chunk, y_max_chunk, z_max_chunk) - min_chunk;

        let volume = dimensions.x * dimensions.y * dimensions.z;
        
        let mut results: Vec<TileData> = Vec::new();

        for i in 0..volume {
            let x = x_min_chunk + i % dimensions.x;
            let y = y_min_chunk + (i / dimensions.x) % dimensions.y;
            let z = z_min_chunk + i / (dimensions.x * dimensions.y);

            let point = Point::new(x,y,z);

            // let mut chunk_query = <(Read<MapChunkData>, Read<Point>)>::query();

            for (map_data, _) in map_datas.iter().filter(|(_,pt)| **pt == point) {
                results.extend(map_data.octree.query_range(range));
            }
        }

        results
    }

    /// Inserts a new mapchunk with the octree data into world
    pub fn insert_mapchunk_with_octree(self, octree: &Octree<i32, TileData>, world: &mut World, changed: bool) -> (Entity, MapChunkData) {
        let map_data = MapChunkData{
            octree: octree.clone(),
        };

        let chunk_pt = map_data.get_chunk_point();

        let area = self.chunk_dimensions.x * self.chunk_dimensions.z;

        if changed {
            (world.push(
                (
                    ManuallyChange(ChangeType::Direct(octree.get_aabb())),
                    chunk_pt,
                    map_data.clone(),
                    #[cfg(not(test))]
                    MeshData::new(),
                    mesh::MapMeshData::new((0..area).map(|_| mesh::VertexData::default()).collect()),
                    custom_mesh::RequiresManualChange{}
                )
            ), map_data)
        } else {
            (world.push(
                (
                    chunk_pt,
                    map_data.clone(),
                    #[cfg(not(test))]
                    MeshData::new(),
                    mesh::MapMeshData::new((0..area).map(|_| mesh::VertexData::default()).collect()),
                    custom_mesh::RequiresManualChange{}
                )
            ), map_data)
        }
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

/// Takes map inputs, determines if they should be added to history (no duplicates), and creates a message if it should
pub fn create_map_input_system() -> impl systems::Runnable {
    SystemBuilder::new("map_input_system")
        .write_resource::<History>()
        .read_resource::<Map>()
        .with_query(<(Entity, Read<MapInput>)>::query())
        .with_query(<(Read<MapChunkData>, Read<Point>)>::query())
        .build(|commands, world, (map_history, map), queries| {

            let (map_input_query, map_data_query) = queries;

            let mut map_messages: Vec<(networking::MessageSender,)> = Vec::new();
            map_input_query.for_each(world, |(entity, map_input)| {

                let map_datas = map_data_query.iter(world).collect::<Vec<(&MapChunkData, &Point)>>();

                let query_range = map.query_chunk_range(map_datas, map_input.octree.get_aabb());

                let input_set = map_input.octree.clone().into_iter().collect::<HashSet<TileData>>();

                //If there is no difference between the input and what is already in the map, just return
                if input_set.symmetric_difference(&query_range.clone().into_iter().collect::<HashSet<TileData>>()).count() == 0 {
                    return {}
                }

                let mut original_state: Octree<i32, TileData> = Octree::new(map_input.octree.get_aabb(), map_input.octree.get_max_elements());

                for item in query_range {
                    original_state.insert(item).unwrap();
                }

                //add the original state to history
                map_history.add_step(StepTypes::MapInput(
                    (
                        MapInput{ 
                            octree: original_state
                        },
                        (*map_input).clone()
                    )
                ));
                //add the new state to history
                // map_history.add_step(StepTypes::MapInput((*map_input).clone()));

                map_messages.push((networking::MessageSender{
                    data_type: networking::DataType::MapInput((*map_input).clone()),
                    message_type: networking::MessageType::Ordered
                },));

                commands.remove(*entity);
            });

            commands.extend(map_messages);
        })
}