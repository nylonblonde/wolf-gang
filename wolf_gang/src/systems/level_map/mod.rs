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
            Octree
        }
    },
    systems::{
        custom_mesh,
        networking::ClientID,
        history::{History, StepType},
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
    Indirect(AABB),
}

///ManuallyChange tells the map chunks to update, and the AABB gives us more information about which columns we will be updating so that we don't have to update all of them. 
/// In most cases, we only need one AABB, but we store it in a Vec for cases where two chunks that are separated by a chunk update simultaneously, effectively overwriting each other's
/// values. This means that ManuallyChange should be attempted to be got with get_component_mut in case it can be updated instead of being written as a new value.
#[derive(Clone, Debug, PartialEq)]
struct ManuallyChange{
    ranges: Vec<ChangeType>
}

/// Message data type for communicating changes over the connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MapChange {
    MapInsertion{
        aabb: AABB,
        tile_data: crate::systems::level_map::TileData
    },
    MapRemoval(AABB),
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
    pub fn change(&self, world: &mut legion::world::World, octree: Octree<i32, TileData>, store_history: Option<u32>) {

        match self.can_change(world, octree.clone()) {
            Err(_) => return,
            Ok((original_state, new_state)) => {
                if let Some(client_id) = store_history {

                    let mut query = <(Write<History>, Read<ClientID>)>::query();

                    if let Some((history, _)) = query.iter_mut(world).filter(|(_, id)| id.val() == client_id).next() {
                        history.add_step(StepType::MapChange((original_state, new_state)));
                    }
                    
                }
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

    /// Returns two octrees: the original state of the map that it compared against on the left, and the new octree input on the right
    pub fn can_change(&self, world: &mut World, octree: Octree<i32, TileData>) -> Result<(Octree<i32, TileData>, Octree<i32, TileData>), Error> {

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

        if existing_octree.clone().into_iter().collect::<HashSet<TileData>>().symmetric_difference(&octree.clone().into_iter().collect::<HashSet<TileData>>()).count() == 0 {
            return Err(Error::new(ErrorKind::AlreadyExists, "There is no symmetric difference between existing data and insertion"));
        }

        Ok((existing_octree, octree))
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

pub fn fill_octree_from_aabb(aabb: AABB, tile_data: Option<TileData>) -> Octree<i32, TileData> {
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