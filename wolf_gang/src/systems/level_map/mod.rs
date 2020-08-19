pub mod mesh;
pub mod document;

use std::sync::mpsc;
use std::collections::HashSet;
use std::collections::HashMap;
use legion::*;
use serde::{Serialize, Deserialize};
use rayon::prelude::*;

use std::io::{Error, ErrorKind};

use crate::{ 
    collections::{
        octree,
        octree::{ 
            Octree, PointData
        }
    },
    systems::{
        custom_mesh,
        history::{History, IsFromHistory, StepTypes},
        networking::{
            ClientID, DataType, MessageSender, MessageType
        }
    },
    node::{NodeName}
};

#[cfg(not(test))]
use crate::systems::custom_mesh::MeshData;

type AABB = crate::geometry::aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;
type Vector3D = nalgebra::Vector3<f32>;


///ChangeType stores the range of the changes so that we can determine whether or not adjacent MapChunks actually need to change, and
/// the range of the original change for making comparisons
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ChangeType {
    Direct(AABB),
    Indirect(AABB)
}

///ManuallyChange tells the map chunks to update, and the AABB gives us more information about which columns we will be updating so that we don't have to update all of them. 
/// In most cases, we only need one AABB, but we store it in a Vec for cases where two chunks that are separated by a chunk update simultaneously, effectively overwriting each other's
/// values. This means that ManuallyChange should be attempted to be got with get_component_mut in case it can be updated instead of being written as a new value.
#[derive(Clone, Debug, PartialEq)]
struct ManuallyChange{
    ranges: Vec<ChangeType>
}

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
pub struct MapChange {
    octree: Octree<i32, TileData>,
    // store_history: bool, //TODO: MapChange should not be handling history
    // ///Whether or not we want to forward the MapChange as a message
    // send: bool,
}

impl MapChange {
    pub fn get_octree(&self) -> &Octree<i32, TileData> {
        &self.octree
    }

    pub fn new(octree: Octree<i32, TileData>, store_history: bool, send: bool) -> Self {
        Self {
            octree,
            // store_history,
            // send,
        }
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

    /// Executes changes to the world map in octree. Takes an optional u32 as a client_id for store_history
    pub fn change(&self, world: &mut legion::world::World, resources: &mut Resources, octree: Octree<i32, TileData>, store_history: bool) {

        if self.can_change(world, octree.clone()).is_err() {
            return
        }

        if store_history {
            if let Some(client_id) = resources.get::<ClientID>() {
                world.push(
                    (MessageSender{
                        data_type: DataType::MapAddHistory{
                            client_id: client_id.val(),
                            aabb: octree.get_aabb()
                        },
                        message_type: MessageType::Ordered
                    },)
                );
            }
        }

        let mut entities: HashMap<Entity, MapChunkData> = HashMap::new();

        let aabb = octree.get_aabb();

        self.range_sliced_to_chunks(aabb).into_iter().for_each(|(pt, _)| {
    
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
        });
    
        for (entity, map_data) in &mut entities {
            
            let map_aabb = map_data.octree.get_aabb();
            let overlap_aabb = aabb.get_intersection(map_aabb);
    
            let map_query_range = map_data.octree.query_range(overlap_aabb);
            let input_query_range = octree.query_range(overlap_aabb);
    
            let set = input_query_range.into_iter().collect::<HashSet<TileData>>();
            let map_set = map_query_range.into_iter().collect::<HashSet<TileData>>();
    
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
    
            // And the range of change to the ManuallyChange component if it exists, otherwise, make it exist
            if let Some(mut entry) = world.entry(*entity) {
                entry.add_component(map_data.clone());

                match entry.get_component_mut::<ManuallyChange>() {
                    Ok(change) => {
                        change.ranges.push(ChangeType::Direct(aabb))
                    },
                    _ => entry.add_component(ManuallyChange{
                        ranges: vec![ChangeType::Direct(aabb)]
                    })
                }
            }
        }
    }

    /// Returns AABBs that are subdivided to fit into the constraints of the chunk dimensions, as well as the chunk pt they'd fit in
    pub fn range_sliced_to_chunks(&self, aabb: AABB) -> Vec<(Point, AABB)> {    
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

        let mut results = Vec::new();
        
        for i in 0..volume {
            let x = x_min_chunk + i % dimensions.x;
            let y = y_min_chunk + (i / dimensions.x) % dimensions.y;
            let z = z_min_chunk + i / (dimensions.x * dimensions.y);
    
            let min = Point::new(x * self.chunk_dimensions.x, y * self.chunk_dimensions.y, z * self.chunk_dimensions.z);
            let max = min + self.chunk_dimensions;

            results.push((Point::new(x,y,z), AABB::from_extents(min, max).get_intersection(aabb)));
        }

        results
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

    pub fn send_remove(&self, world: &mut legion::world::World, aabb:AABB) -> Result<(), Error> {
        match self.can_change(world, self.fill_octree_from_aabb(aabb, None)) {
            Err(err) => return Err(err),
            _ => {
                world.push(
                    (
                        MessageSender{
                            data_type: DataType::MapRemoval(aabb),
                            message_type: MessageType::Ordered
                        },
                    )
                );

                Ok({})
            }
        }
    }

    pub fn remove(&self, world: &mut legion::world::World, resources: &mut Resources, aabb: AABB) -> Result<(), Error> {
        match self.can_change(world, self.fill_octree_from_aabb(aabb, None)) {
            Err(err) => return Err(err),
            Ok(octree) => {

                self.change(world, resources, octree, true);
                // world.push(
                //     (
                //         MapChange {
                //             octree,
                //             // store_history: true,
                //             // send: false
                //         },
                //     )
                // );

                Ok({})
            }
        }
    }

    pub fn send_insert(&self, world: &mut legion::world::World, tile_data: TileData, aabb: AABB) -> Result<(), Error> {
        
        match self.can_change(world, self.fill_octree_from_aabb(aabb, Some(tile_data))) {
            Err(err) => return Err(err),
            _ => {
                world.push(
                    (
                        MessageSender{
                            data_type: DataType::MapInsertion{
                                aabb, tile_data
                            },
                            message_type: MessageType::Ordered
                        },
                    )
                );
                
                Ok({})
            }
        }

    }

    pub fn insert(&self, world: &mut legion::world::World, resources: &mut Resources, tile_data: TileData, aabb: AABB) -> Result<(), Error> {
        match self.can_change(world, self.fill_octree_from_aabb(aabb, Some(tile_data))) {
            Err(err) => return Err(err),
            Ok(octree) => {

                self.change(world, resources, octree, true);
                // world.push(
                //     (
                //         MapChange {
                //             octree,
                //             // store_history: true,
                //             // send: false
                //         },
                //     )
                // );

                Ok({})
            }
        }
        
    }

    /// Does a query range on every chunk that fits within the range
    pub fn query_chunk_range<'a, T: IntoIterator<Item=(Entity, MapChunkData, Point)> + Clone>(&self, map_datas: T, range: AABB) -> Vec<TileData> {
    
        let mut results = Vec::new();

        self.chunks_in_range(map_datas, range).iter().for_each(|(_, map_data)| {
            results.extend(map_data.octree.query_range(range))
        });

        results
    }

    pub fn chunks_in_range<'a, T: IntoIterator<Item=(Entity, MapChunkData, Point)> + Clone>(&self, map_datas: T, range: AABB) -> Vec<(Entity, MapChunkData)> {
        
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
        
        let mut results: Vec<(Entity, MapChunkData)> = Vec::new();

        for i in 0..volume {
            let x = x_min_chunk + i % dimensions.x;
            let y = y_min_chunk + (i / dimensions.x) % dimensions.y;
            let z = z_min_chunk + i / (dimensions.x * dimensions.y);

            let point = Point::new(x,y,z);

            map_datas.clone().into_iter().filter(|(_, _, pt)| *pt == point).for_each(|(entity, map_data, _)| {
                results.push((entity, map_data));
            });

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
                    ManuallyChange{
                        ranges: vec![ChangeType::Direct(octree.get_aabb())]
                    },
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

    fn fill_octree_from_aabb(&self, aabb: AABB, tile_data: Option<TileData>) -> Octree<i32, TileData> {
        let mut octree = Octree::new(aabb, octree::DEFAULT_MAX);

        let min = aabb.get_min();

        let dimensions = aabb.dimensions.abs();

        let volume = dimensions.x * dimensions.y * dimensions.z;

        let (tx, rx) = mpsc::channel::<TileData>();

        if let Some(tile_data) = tile_data {
            (0..volume).into_par_iter().for_each_with(tx, move |tx, i| {
                let x = min.x + i % dimensions.x;
                let y = min.y + (i / dimensions.x) % dimensions.y;
                let z = min.z + i / (dimensions.x * dimensions.y);

                tx.send(TileData {
                    point: Point::new(x,y,z),
                    ..tile_data
                }).unwrap();
            });
        }

        rx.try_iter().for_each(|item| {
            octree.insert(item).ok();
        });

        octree

    }

    pub fn can_change(&self, world: &mut World, octree: Octree<i32, TileData>) -> Result<Octree<i32, TileData>, Error> {

        let aabb = octree.get_aabb();

        let mut map_query = <(Entity, Read<MapChunkData>, Read<Point>)>::query();
        let results = map_query.iter(world)
            .map(|(entity, map_data, pt)| (*entity, (*map_data).clone(), *pt))
            .collect::<Vec<(Entity, MapChunkData, Point)>>();

        let existing_data = self.query_chunk_range(results, aabb);

        let mut existing_octree = Octree::new(aabb, octree::DEFAULT_MAX);

        existing_data.into_iter().for_each(|tile_data| {
            existing_octree.insert(tile_data).ok();
        });

        if existing_octree.into_iter().collect::<HashSet<TileData>>().symmetric_difference(&octree.clone().into_iter().collect::<HashSet<TileData>>()).count() == 0 {
            return Err(Error::new(ErrorKind::AlreadyExists, "There is no symmetric difference between existing data and insertion"));
        }

        Ok(octree)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapChunkData {
    pub octree: Octree<i32, TileData>,
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

// /// Takes map changes, determines if they should be added to history (no duplicates), and creates a message if it should
// pub fn create_map_change_system() -> impl systems::Runnable {
//     SystemBuilder::new("map_input_system")
//         // .write_resource::<History>()
//         .read_resource::<ClientID>()
//         .read_resource::<Map>()
//         .read_component::<IsFromHistory>()
//         .with_query(<(Entity, Read<MapChange>)>::query())
//         .with_query(<(Entity, Read<MapChunkData>, Read<Point>)>::query())
//         // .with_query(<(Write<History>, Read<ClientID>)>::query())
//         .build(|commands, world, (client_id, map), queries| {

//             let (map_change_query, map_data_query) = queries;

//             let map_changes = map_change_query.iter(world)
//                 .map(|(entity, map_change)| (*entity, (*map_change).clone()))
//                 .collect::<Vec<(Entity, MapChange)>>();

//             let map_datas = map_data_query.iter(world)
//                 .map(|(entity, map_data, pt)| (*entity, (*map_data).clone(), *pt))
//                 .collect::<Vec<(Entity, MapChunkData, Point)>>();

//             let mut map_messages: Vec<(MessageSender,)> = Vec::new();
//             let mut octrees = Vec::new();

//             map_changes.into_iter().for_each(|(entity, map_change)| {
//                 commands.remove(entity);

//                 let aabb = map_change.octree.get_aabb();

//                 let query_range = map.query_chunk_range(map_datas.clone(), aabb);

//                 let input_set = map_change.octree.clone().into_iter().collect::<HashSet<TileData>>();

//                 //If there is no difference between the input and what is already in the map, just return
//                 if input_set.symmetric_difference(&query_range.clone().into_iter().collect::<HashSet<TileData>>()).count() == 0 {
//                     return {}
//                 }

//                 if let Some(entry) = world.entry_ref(entity) {

//                     // Only add to history if this entity does not contain an IsFromHistory component
//                     if entry.get_component::<IsFromHistory>().is_err() {

//                     // if let Some((history, _)) = history_query.iter_mut(world).filter(|(_, id)| id.val() == client_id.val() ).next() {
//                     //     let mut original_state: Octree<i32, TileData> = Octree::new(map_change.octree.get_aabb(), octree::DEFAULT_MAX);

//                     //     for item in query_range {
//                     //         original_state.insert(item).unwrap();
//                     //     }

//                         // history.add_step(StepTypes::MapChange(
//                         //     (
//                         //         MapChange{ 
//                         //             octree: original_state,
//                         //             // store_history: false,
//                         //             // send: true
//                         //         },
//                         //         MapChange{
//                         //             octree: map_change.octree.clone(),
//                         //             // store_history: false,
//                         //             // send: true
//                         //         }
//                         //     )
//                         // ));
//                     // }                    
                        
//                     }

//                     map_messages.push(
//                         (
//                             MessageSender {
//                                 data_type: DataType::MapAddHistory{
//                                     aabb,
//                                     client_id: client_id.val()
//                                 },
//                                 message_type: MessageType::Ordered,
//                             },
//                         )
//                     );

//                     octrees.push(map_change.octree.clone());

//                 }


//                 // let input_as_chunks = map.range_sliced_to_chunks(input_aabb);
//                 // let num_affected_cols: i32 = input_as_chunks.iter().map(|(_, aabb)| aabb.dimensions.x * aabb.dimensions.z).sum();
                
//                 // if map_input.send {
//                 //     // If we'd have to update more columns than a single chunk would have, let's split that up, otherwise just send the whole map input
//                 //     if num_affected_cols > map.chunk_dimensions.x * map.chunk_dimensions.z {
            
//                 //         input_as_chunks.iter().for_each(|(point, aabb)| {
            
//                 //             // let octree = map_input.octree.query_range(*aabb).into_iter().collect::<Octree<i32, TileData>>();
//                 //             let mut octree = Octree::new(*aabb, octree::DEFAULT_MAX);

//                 //             input_set.iter().for_each(|tile_data| {
//                 //                 if octree.get_aabb().contains_point(tile_data.get_point()) {
//                 //                     octree.insert(*tile_data).unwrap();
//                 //                 }
//                 //             });
                            
//                 //             // Check if there is an existing chunk at the point, and if there is, return if there is no symmetric difference as there's no need to update
//                 //             if let Some(existing_chunk) = map_datas.clone().into_iter()
//                 //                 .filter(|(_,_,pt)| **pt == *point)
//                 //                 .map(|(_, map_data, _)| map_data)
//                 //                 .next() 
//                 //             {
            
//                 //                 let slice_of_chunk = existing_chunk.octree.query_range(*aabb).into_iter().collect::<HashSet<TileData>>();
            
//                 //                 if octree.clone().into_iter().collect::<HashSet<TileData>>().symmetric_difference(&slice_of_chunk).count() == 0 {
//                 //                     return {}
//                 //                 }
//                 //             }
            
//                 //             if map_input.send {
//                 //                 map_messages.push((networking::MessageSender{
//                 //                     data_type: networking::DataType::MapChange(
//                 //                         octree
//                 //                     ),
//                 //                     message_type: networking::MessageType::Ordered
//                 //                 },));
//                 //             } 
                            
//                 //         });
            
//                 //     } else {
            
//                 //         if map_input.send {
//                 //             map_messages.push((networking::MessageSender{
//                 //                 data_type: networking::DataType::MapChange(map_input.octree.clone()),
//                 //                 message_type: networking::MessageType::Ordered
//                 //             },));
//                 //         } 
                        
//                 //     } 
//                 // } else {

//                 //     let map = **map;
//                 //     let octree = map_input.octree.clone();

//                 //     commands.exec_mut(move |world| {
//                 //         map.change(world, octree.clone());    
//                 //     });
//                 // }
        
//             });


//             let map = **map;

//             commands.exec_mut(move |world| {
//                 octrees.clone().into_iter().for_each(|octree| {
//                     map.change(world, octree);
//                 });
//             });

//             commands.extend(map_messages);
//         })
// }
