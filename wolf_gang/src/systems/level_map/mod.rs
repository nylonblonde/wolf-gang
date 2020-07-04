pub mod mesh;
pub mod history;
pub mod document;

use gdnative::godot_print;

use std::collections::HashMap;
use legion::prelude::*;
use serde::{Serialize, Deserialize};

use crate::{ 
    collections::octree::Octree,
};

#[cfg(not(test))]
use crate::custom_mesh::MeshData;

type AABB = crate::geometry::aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;
type Vector3D = nalgebra::Vector3<f32>;


#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ChangeType {
    Direct,
    Indirect
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

pub struct Map {
    chunk_dimensions: Point,
    // map_chunk_pool: HashMap<Point, MapChunkData>
}

impl Default for Map {
    fn default() -> Self {
        Map { 
            // map_chunk_pool: HashMap::new(),
            chunk_dimensions: Point::new(10,10,10)
        }
    }
}

impl Map {

    /// Deletes all entities for the map chunks, removes the mesh nodes from the node cache, and resets the Document and CurrentHistoricalStep resources
    pub fn reset(&self, world: &mut legion::world::World, resources: &Resources) {

        let mut current_step = resources.get_mut::<crate::history::CurrentHistoricalStep>().unwrap();
        let mut document = resources.get_mut::<document::Document>().unwrap();

        let map_chunk_query = <(Read<MapChunkData>, Tagged<crate::node::NodeName>)>::query();

        let mut entities: Vec<Entity> = Vec::new();

        for (entity, (_, node_name)) in map_chunk_query.iter_entities(world) {
            entities.push(entity);

            unsafe { crate::node::remove_node(node_name.0.clone()); }
        }

        for entity in entities {
            world.delete(entity);
        }

        *current_step = crate::history::CurrentHistoricalStep::default();
        *document = document::Document::default();
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

            if map_data != original {
                world.add_component(*entity, map_data.clone()).unwrap();
                world.add_tag(*entity, ManuallyChange(ChangeType::Direct)).unwrap();

                historically_significant.insert(*entity, map_data);
            }
        }

        history::add_to_history(world, current_step, &mut historically_significant, CoordPos { value: aabb.center }, aabb);

    }

    pub fn insert(&self, world: &mut legion::world::World, current_step: &mut crate::history::CurrentHistoricalStep, tile_data: TileData, aabb: AABB) {

        let min = aabb.get_min();
        let max = aabb.get_max();

        let x_min_chunk = (min.x as f32 / self.chunk_dimensions.x as f32).floor() as i32;
        let y_min_chunk = (min.y as f32 / self.chunk_dimensions.y as f32).floor() as i32;
        let z_min_chunk = (min.z as f32 / self.chunk_dimensions.z as f32).floor() as i32;

        let x_max_chunk = (max.x as f32/ self.chunk_dimensions.x as f32).floor() as i32 + 1;
        let y_max_chunk = (max.y as f32/ self.chunk_dimensions.y as f32).floor() as i32 + 1;
        let z_max_chunk = (max.z as f32/ self.chunk_dimensions.z as f32).floor() as i32 + 1;

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
                    entities.insert(entity, (*map_chunk).clone());
                    exists = true;
                },
                _ => {}
            }

            if !exists {
                println!("Creating a new map chunk at {:?}", pt);

                let map_data = MapChunkData{
                    octree: Octree::new(AABB::new(
                        Point::new(
                            pt.x * self.chunk_dimensions.x + self.chunk_dimensions.x/2,
                            pt.y * self.chunk_dimensions.y + self.chunk_dimensions.y/2,
                            pt.z * self.chunk_dimensions.z + self.chunk_dimensions.z/2,
                        ),
                        self.chunk_dimensions
                    ))
                };

                let entity = world.insert((pt,),vec![
                    (
                        map_data.clone(),
                        history::MapChunkHistory::new(map_data.clone()),
                        #[cfg(not(test))]
                        MeshData::new(),
                    )
                ])[0];

                entities.insert(entity, map_data);
            }
        }

        for (entity, map_data) in &mut entities {

            let original = map_data.clone();

            let chunk_aabb = map_data.octree.get_aabb();
            let chunk_min = chunk_aabb.get_min();
            let chunk_max = chunk_aabb.get_max();

            let min_x = std::cmp::max(chunk_min.x, min.x);
            let min_y = std::cmp::max(chunk_min.y, min.y);
            let min_z = std::cmp::max(chunk_min.z, min.z);

            let max_x = std::cmp::min(chunk_max.x, max.x) + 1;
            let max_y = std::cmp::min(chunk_max.y, max.y) + 1;
            let max_z = std::cmp::min(chunk_max.z, max.z) + 1;

            let min = Point::new(min_x, min_y, min_z);
            let dimensions = Point::new(max_x, max_y, max_z) - min;
            let volume = dimensions.x * dimensions.y * dimensions.z;

            for i in 0..volume {
                let x = min_x + i % dimensions.x;
                let y = min_y + (i / dimensions.x) % dimensions.y;
                let z = min_z + i / (dimensions.x * dimensions.y);
            
                let pt = Point::new(x,y,z);

                match map_data.octree.insert(TileData{
                    point: pt,
                    ..tile_data
                }) {
                    Ok(_) => {
                    // println!("Inserted {:?}", pt);
                    },
                    Err(err) => {
                        println!("{:?}", err);
                    }
                }
            }

            if *map_data != original {
                let map_data = map_data.clone();

                world.add_component(*entity, map_data.clone()).unwrap();
                world.add_tag(*entity, ManuallyChange(ChangeType::Direct)).unwrap();

                historically_significant.insert(*entity, map_data);
            }
                
        }

        history::add_to_history(world, current_step, &mut historically_significant, CoordPos { value: aabb.center }, aabb);

    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapChunkData {
    octree: Octree<i32, TileData>,
}

impl MapChunkData {
    pub fn new(aabb: AABB) -> Self {
        MapChunkData {
            octree: Octree::new(aabb)
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

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
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