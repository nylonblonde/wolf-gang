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

pub fn create_system() -> Box<dyn Schedulable> {
    SystemBuilder::<()>::new("map_system")
            .with_query(<(Read<MapChunkData>, Write<custom_mesh::MeshData>)>::query()
                .filter(changed::<MapChunkData>())
            )
            .build(move |commands, world, resource, queries| {
                for (map_data, mut mesh_data) in queries.iter_mut(&mut *world) {
                    godot_print!("{:?}", "there should only be one tick");
                    mesh_data.verts = Vector3Array::new();
                    mesh_data.normals = Vector3Array::new();
                    mesh_data.uvs = Vector2Array::new();
                    mesh_data.indices = Int32Array::new();

                    let mut offset = 0;
                    for tile in map_data.octree.clone().into_iter() {

                        let point = tile.get_point();

                        godot_print!("drawing {:?}", point);

                        let point = map_coords_to_world(point);

                        mesh_data.verts.push(&Vector3::new(point.x, point.y+TILE_DIMENSIONS.y, point.z));
                        mesh_data.verts.push(&Vector3::new(point.x+TILE_DIMENSIONS.x, point.y+TILE_DIMENSIONS.y, point.z+TILE_DIMENSIONS.z));
                        mesh_data.verts.push(&Vector3::new(point.x, point.y+TILE_DIMENSIONS.y, point.z+TILE_DIMENSIONS.z));
                        mesh_data.verts.push(&Vector3::new(point.x+TILE_DIMENSIONS.x, point.y+TILE_DIMENSIONS.y, point.z));

                        mesh_data.uvs.push(&Vector2::new(0.,1.));
                        mesh_data.uvs.push(&Vector2::new(1.,1.));
                        mesh_data.uvs.push(&Vector2::new(0.,0.));
                        mesh_data.uvs.push(&Vector2::new(1.,0.));

                        mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                        mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                        mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                        mesh_data.normals.push(&Vector3::new(0.,1.,0.));

                        mesh_data.indices.push(offset);
                        mesh_data.indices.push(offset+1);
                        mesh_data.indices.push(offset+2);

                        mesh_data.indices.push(offset+1);
                        mesh_data.indices.push(offset);
                        mesh_data.indices.push(offset+3);

                        offset += 4;
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

impl Map {
    pub fn new() -> Self {
        Map { 
            // map_chunk_pool: HashMap::new(),
            chunk_dimensions: Point::new(10,10,10)
        }
    }

    pub fn insert(&self, world: &mut legion::world::World, tile_data: TileData, aabb: AABB) {

        let min = aabb.get_min();
        let max = aabb.get_max();

        godot_print!("{:?} {:?}", min, max);

        let x_max = min.x + aabb.dimensions.x;
        let y_max = min.y + aabb.dimensions.y;
        let z_max = min.z + aabb.dimensions.z;

        let x_min_chunk = (min.x as f32 / self.chunk_dimensions.x as f32).floor() as i32;
        let y_min_chunk = (min.y as f32 / self.chunk_dimensions.y as f32).floor() as i32;
        let z_min_chunk = (min.z as f32 / self.chunk_dimensions.z as f32).floor() as i32;

        let x_max_chunk = (x_max as f32/ self.chunk_dimensions.x as f32).floor() as i32 + 1;
        let y_max_chunk = (y_max as f32/ self.chunk_dimensions.y as f32).floor() as i32 + 1;
        let z_max_chunk = (z_max as f32/ self.chunk_dimensions.z as f32).floor() as i32 + 1;

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
                            entities.insert(entity);
                            exists = true;
                        },
                        _ => {}
                    }

                    if !exists {

                        godot_print!("Creating a new map chunk at {:?}", pt);

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
                                custom_mesh::MeshData::new()
                            )
                        ])[0];

                        entities.insert(entity);
                    }
                }
            }
        }

        let query = <Write<MapChunkData>>::query();

        for (entity, mut map_chunk) in query.iter_entities_mut(&mut *world) {
            if !entities.contains(&entity) {
                continue
            }

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

                        godot_print!("Inserting {:?}", pt);

                        if map_chunk.octree.insert(TileData{
                            point: pt,
                            ..tile_data
                        }) {
                            godot_print!("Inserted {:?}", pt);
                        }

                    }
                }
            }

        }

        // let mut to_add: HashMap<Entity, MapChunkData> = HashMap::new();

        // for entity in entities {
        //     let map_chunk = world.get_component_mut::<MapChunkData>(entity);

        //     match map_chunk {
        //         Some(mut r) => {

        //             let chunk_aabb = r.octree.get_aabb();
        //             let chunk_min = chunk_aabb.get_min();
        //             let chunk_max = chunk_aabb.get_max();

        //             let min_x = std::cmp::max(chunk_min.x, min.x);
        //             let min_y = std::cmp::max(chunk_min.y, min.y);
        //             let min_z = std::cmp::max(chunk_min.z, min.z);

        //             let max_x = std::cmp::min(chunk_max.x, max.x);
        //             let max_y = std::cmp::min(chunk_max.y, max.y);
        //             let max_z = std::cmp::min(chunk_max.z, max.z);

        //             for z in min_z..max_z {
        //                 for y in min_y..max_y {
        //                     for x in min_x..max_x {

        //                         let pt = Point::new(x,y,z);

        //                         godot_print!("Inserting {:?}", pt);

        //                         r.octree.insert(TileData{
        //                             point: pt,
        //                             ..tile_data
        //                         });

        //                     }
        //                 }
        //             }
        //             to_add.insert(entity, r.clone());

        //         },
        //         None => {}
        //     }
        // }

        // for (entity, map_chunk) in to_add {

        //     godot_print!("adding {:?} to {:?}", map_chunk.octree.get_aabb().center, entity);

        //     world.add_component(entity, map_chunk).unwrap();
        // }

    }


    // pub fn insert(self: &Self, world: &mut legion::world::World, tile_data: TileData) {

    //     let point = tile_data.get_point();

    //     //Matrix<i32> doesn't implement div so we gotta do it manually
    //     //Gotta convert to and from floats to ensure that we get the floor as negative ints round to ceiling
    //     let chunk_point = Point::new(
    //         (point.x as f32 / self.chunk_dimensions.x as f32).floor() as i32,
    //         (point.y as f32 / self.chunk_dimensions.y as f32).floor() as i32,
    //         (point.z as f32 / self.chunk_dimensions.z as f32).floor() as i32,
    //     );

    //     let map_chunk_query = <Write<MapChunkData>>::query()
    //         .filter(tag_value(&chunk_point));

    //     let mut exists: bool = false;

    //     // for mut map_chunk in map_chunk_query.iter_mut(&mut *world){
    //     match map_chunk_query.iter_mut(world).next() {
    //         Some(mut map_chunk) => {
    //             if map_chunk.octree.insert(tile_data) {
    //                 godot_print!("Inserted {:?} tile data into the existing map_chunk at {:?}", point, chunk_point);
    //                 exists = true; 
    //             } else {
    //                 godot_print!("Failed to insert {:?} tile data into the existing map_chunk at {:?}", point, chunk_point);
    //             }
    //         },
    //         None => {} 
    //     }
    //     // }

    //     if exists { return }

    //     let center = Point::new(
    //         chunk_point.x * self.chunk_dimensions.x + self.chunk_dimensions.x/2,
    //         chunk_point.y * self.chunk_dimensions.y + self.chunk_dimensions.y/2,
    //         chunk_point.z * self.chunk_dimensions.z + self.chunk_dimensions.z/2,
    //     );

    //     let mut map_chunk = MapChunkData{
    //         octree: Octree::new(AABB::new(center, self.chunk_dimensions))
    //     };

    //     if map_chunk.octree.insert(tile_data){
    //         godot_print!("Inserted {:?} tile data into the new map_chunk at {:?}", point, chunk_point);

    //         world.insert((chunk_point.clone(),), vec![
    //             (map_chunk, custom_mesh::MeshData::new()),
    //         ]);
    //     } else {
    //         godot_print!("Failed to insert {:?} tile data into the new map chunk at {:?}", point, chunk_point);
    //     }
    // }
}

#[derive(Clone)]
pub struct MapChunkData {
    octree: Octree<i32, TileData>,
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

