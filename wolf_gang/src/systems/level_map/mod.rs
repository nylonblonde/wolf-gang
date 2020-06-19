pub mod mesh;
pub mod history;

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

#[derive(Copy, Clone)]
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

        let mut to_update: Vec<Entity> = Vec::new();

        for i in 0..volume {
            let x = x_min_chunk + i % dimensions.x;
            let y = y_min_chunk + (i / dimensions.x) % dimensions.y;
            let z = z_min_chunk + i / (dimensions.x * dimensions.y);

            let pt = Point::new(x,y,z);

            let map_chunk_exists_query = <Write<MapChunkData>>::query()
                .filter(tag_value(&pt));

            if let Some((entity, mut map_data)) = map_chunk_exists_query.iter_entities_mut(world).next() {
                map_data.octree.remove_range(aabb);
                to_update.push(entity);
            }
        }

        for entity in to_update {
            world.add_tag(entity, ManuallyChange(ChangeType::Direct)).unwrap();

            let mut map_chunk: Option<MapChunkData> = None;

            match world.get_component_mut::<MapChunkData>(entity) {
                Some(mut r) => {
                    map_chunk = Some(r.clone());
                },
                None => {}
            }

            if let Some(map_chunk) = map_chunk {
                historical_account(world, *current_step, entity, map_chunk, CoordPos { value: aabb.center });
            }
        }

        current_step.0 += 1;

        godot_print!("{:?}", current_step);

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

        let mut entities: Vec<Entity> = Vec::new();

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
                Some((entity, _)) => {
                    println!("Map chunk exists already");
                    entities.push(entity);
                    exists = true;
                },
                _ => {}
            }

            if !exists {
                println!("Creating a new map chunk at {:?}", pt);

                let entity = world.insert((pt,),vec![
                    (
                        MapChunkData{
                            octree: Octree::new(AABB::new(
                                Point::new(
                                    pt.x * self.chunk_dimensions.x + self.chunk_dimensions.x/2,
                                    pt.y * self.chunk_dimensions.y + self.chunk_dimensions.y/2,
                                    pt.z * self.chunk_dimensions.z + self.chunk_dimensions.z/2,
                                ),
                                self.chunk_dimensions
                            ))
                        },
                        history::MapChunkHistory::new(),
                        #[cfg(not(test))]
                        MeshData::new(),
                    )
                ])[0];

                entities.push(entity);
            }

        }

        let mut to_add: HashMap<Entity, MapChunkData> = HashMap::new();

        for entity in entities {
            let map_chunk = world.get_component_mut::<MapChunkData>(entity);

            match map_chunk {
                Some(mut map_chunk) => {
                    let chunk_aabb = map_chunk.octree.get_aabb();
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

                        match map_chunk.octree.insert(TileData{
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

                    to_add.insert(entity, map_chunk.clone());
                },
                None => {}
            }
        }

        for (entity, map_chunk) in to_add {
            world.add_component(entity, map_chunk.clone()).unwrap();
            world.add_tag(entity, ManuallyChange(ChangeType::Direct)).unwrap();

            historical_account(world, *current_step, entity, map_chunk, CoordPos { value: aabb.center });
        }

        current_step.0 += 1;

        godot_print!("{:?}", current_step);
    }
}

/// Pushes the map chunk changes to the entity's history to be able to handle undo/redo
pub fn historical_account(world: &mut World, current_step: crate::history::CurrentHistoricalStep, entity: Entity, map_chunk_data: MapChunkData, coord_pos: CoordPos) {
    
    let mut map_chunk_history: Option<history::MapChunkHistory> = None;
    match world.get_component_mut::<history::MapChunkHistory>(entity) {
        Some(mut r) => {
            r.steps.push(history::MapChunkChange{
                coord_pos,
                step_changed_at: current_step,
                map_chunk_data
            });

            map_chunk_history = Some(r.clone());
        },
        None => {}
    }

    if let Some(map_chunk_history) = map_chunk_history {
        world.add_component(entity, map_chunk_history).unwrap();
    }
}

#[derive(Clone)]
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