use std::collections::HashSet;
use crate::custom_mesh; 
use crate::geometry::aabb;
use crate::collections::octree::{Octree, PointData};

use gdnative::*;

use nalgebra;

use legion::prelude::*;

use std::collections::HashMap;

type AABB = aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;

pub struct TileDimensions {
    pub x: f32,
    pub y: f32,
    pub z: f32
}

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

pub const TILE_DIMENSIONS: TileDimensions = TileDimensions {x: 1.0, y: 0.2, z: 1.0};

pub fn create_add_material_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("map_add_material_system")
        .with_query(<Read<custom_mesh::MeshData>>::query()
            .filter(component::<MapChunkData>() & !component::<custom_mesh::Material>())
        )
        .build(move |commands, world, _, query| {
            for (entity, _) in query.iter_entities(&mut *world) {
                commands.exec_mut(move |world| {
                    match world.add_component(entity, custom_mesh::Material::from_str("res://materials/ground.material")) {
                        Ok(_) => { godot_print!("Added material to ground!"); },
                        _ => { godot_print!("Couldn't attach material to level map!"); }
                    }
                });
            }
        })
}

pub fn create_drawing_thread_local_fn() -> Box<dyn FnMut(&mut legion::world::World, &mut Resources)>  {
    let write_mesh_query = <(Read<MapChunkData>, Write<custom_mesh::MeshData>)>::query()
        .filter(changed::<MapChunkData>());
    
    Box::new(move |world, _| {

        unsafe {
            for (map_data, mut mesh_data) in write_mesh_query.iter_unchecked(world) {
                godot_print!("Drawing {:?}", map_data.get_chunk_point());
                mesh_data.verts = Vector3Array::new();
                mesh_data.normals = Vector3Array::new();
                mesh_data.uvs = Vector2Array::new();
                mesh_data.indices = Int32Array::new();

                let points_done: HashSet::<Point> = HashSet::new();

                let mut offset = 0;
                for tile in map_data.octree.clone().into_iter() {

                    let point = tile.get_point();
                    if points_done.contains(&point){
                        continue;
                    }

                    let point_above = point + Point::y();

                    if map_data.octree.clone().query_point(point_above).is_some() {
                        continue;
                    }

                    let chunk_point_above = map_data.get_chunk_point()+Point::y();

                    let map_data_above_query = <Read<MapChunkData>>::query()
                        .filter(tag_value(&chunk_point_above));

                    match map_data_above_query.iter_unchecked(world).next() {
                        Some(map_data_above) => {
                            if map_data_above.octree.clone().query_point(point_above).is_some() {
                                continue;
                            }
                        },
                        None => {}
                    }

                    // godot_print!("drawing {:?}", point);

                    let point = map_coords_to_world(point);
                    mesh_data.verts.push(&Vector3::new(point.x, point.y+TILE_DIMENSIONS.y, point.z+TILE_DIMENSIONS.z));
                    mesh_data.verts.push(&Vector3::new(point.x+TILE_DIMENSIONS.x, point.y+TILE_DIMENSIONS.y, point.z+TILE_DIMENSIONS.z));
                    mesh_data.verts.push(&Vector3::new(point.x, point.y+TILE_DIMENSIONS.y, point.z));
                    mesh_data.verts.push(&Vector3::new(point.x+TILE_DIMENSIONS.x, point.y+TILE_DIMENSIONS.y, point.z));

                    mesh_data.uvs.push(&Vector2::new(0.,0.));
                    mesh_data.uvs.push(&Vector2::new(1.,0.));
                    mesh_data.uvs.push(&Vector2::new(0.,1.));
                    mesh_data.uvs.push(&Vector2::new(1.,1.));

                    mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                    mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                    mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                    mesh_data.normals.push(&Vector3::new(0.,1.,0.));

                    mesh_data.indices.push(offset+2);
                    mesh_data.indices.push(offset+1);
                    mesh_data.indices.push(offset);

                    mesh_data.indices.push(offset+3);
                    mesh_data.indices.push(offset+1);
                    mesh_data.indices.push(offset+2);

                    offset += 4;
                }
            }
        }
    })
} 

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

    pub fn insert(&self, world: &mut legion::world::World, tile_data: TileData, aabb: AABB) {

        let min = aabb.get_min();
        let max = aabb.get_max();

        println!("center: {:?} dim: {:?}", aabb.center, aabb.dimensions);

        let x_min_chunk = (min.x as f32 / self.chunk_dimensions.x as f32).floor() as i32;
        let y_min_chunk = (min.y as f32 / self.chunk_dimensions.y as f32).floor() as i32;
        let z_min_chunk = (min.z as f32 / self.chunk_dimensions.z as f32).floor() as i32;

        let x_max_chunk = (max.x as f32/ self.chunk_dimensions.x as f32).floor() as i32 + 1;
        let y_max_chunk = (max.y as f32/ self.chunk_dimensions.y as f32).floor() as i32 + 1;
        let z_max_chunk = (max.z as f32/ self.chunk_dimensions.z as f32).floor() as i32 + 1;

        let mut entities: HashSet<Entity> = HashSet::new();

        for z in z_min_chunk..z_max_chunk {
            for y in y_min_chunk..y_max_chunk {
                for x in x_min_chunk..x_max_chunk {

                    let pt = Point::new(x,y,z);
                    let map_chunk_exists_query = <Read<MapChunkData>>::query()
                        .filter(tag_value(&pt));

                    let mut exists = false;
                    match map_chunk_exists_query.iter_entities(&mut *world).next() {
                        Some((entity, _)) => {
                            println!("Map chunk exists already");
                            entities.insert(entity);
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
                                #[cfg(not(test))]
                                custom_mesh::MeshData::new()
                            )
                        ])[0];

                        entities.insert(entity);
                    }
                }
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

                    let max_x = std::cmp::min(chunk_max.x, max.x);
                    let max_y = std::cmp::min(chunk_max.y, max.y);
                    let max_z = std::cmp::min(chunk_max.z, max.z);

                    for z in min_z..max_z {
                        for y in min_y..max_y {
                            for x in min_x..max_x {

                                let pt = Point::new(x,y,z);

                                if map_chunk.octree.insert(TileData{
                                    point: pt,
                                    ..tile_data
                                }) {
                                    // println!("Inserted {:?}", pt);
                                }
                            }
                        }
                    }

                    to_add.insert(entity, map_chunk.clone());
                },
                None => {}
            }
        }

        for (entity, map_chunk) in to_add {
            world.add_component(entity, map_chunk).unwrap();
        }

        //TODO: This doesn't work, we need to filter it so only the edited chunks update.
        // let query = <Write<MapChunkData>>::query();
        // let query = query.clone().filter(tag_value(&1u32));

        // for (entity, mut map_chunk) in query.iter_entities_mut(&mut *world) {
        //     if !entities.contains(&entity) {
        //         continue
        //     }

        //     let chunk_aabb = map_chunk.octree.get_aabb();
        //     let chunk_min = chunk_aabb.get_min();
        //     let chunk_max = chunk_aabb.get_max();

        //     let min_x = std::cmp::max(chunk_min.x, min.x);
        //     let min_y = std::cmp::max(chunk_min.y, min.y);
        //     let min_z = std::cmp::max(chunk_min.z, min.z);

        //     let max_x = std::cmp::min(chunk_max.x, max.x);
        //     let max_y = std::cmp::min(chunk_max.y, max.y);
        //     let max_z = std::cmp::min(chunk_max.z, max.z);

        //     for z in min_z..max_z {
        //         for y in min_y..max_y {
        //             for x in min_x..max_x {

        //                 let pt = Point::new(x,y,z);

        //                 // godot_print!("Inserting {:?}", pt);

        //                 if map_chunk.octree.insert(TileData{
        //                     point: pt,
        //                     ..tile_data
        //                 }) {
        //                     // godot_print!("Inserted {:?}", pt);
        //                 }
        //             }
        //         }
        //     }
        // }
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

#[derive(Clone)]
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

