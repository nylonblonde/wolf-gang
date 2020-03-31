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

pub const TILE_DIMENSIONS: TileDimensions = TileDimensions {x: 1.0, y: 0.25, z: 1.0};
pub const TILE_PIXELS: f32 = 64.;
pub const SHEET_PIXELS: f32 = 1024.;
pub const TILE_SIZE: f32 = TILE_PIXELS/SHEET_PIXELS;

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
        // .filter(changed::<MapChunkData>())
            .filter(tag_value(&ManuallyChange(true)))
        ;
    
    Box::new(move |world, _| {

        let mut changed: Vec<Entity> = Vec::new();

        unsafe {
            for (entity, (map_data, mut mesh_data)) in write_mesh_query.iter_entities_unchecked(world) {

                changed.push(entity);

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

                    if map_data.octree.query_point(point_above).is_some() {
                        continue;
                    }

                    let chunk_point_above = map_data.get_chunk_point()+Point::y();

                    let map_data_above_query = <Read<MapChunkData>>::query()
                        .filter(tag_value(&chunk_point_above));

                    match map_data_above_query.iter_unchecked(world).next() {
                        Some(map_data_above) => {
                            if map_data_above.octree.query_point(point_above).is_some() {
                                continue;
                            }
                        },
                        None => {}
                    }

                    // godot_print!("drawing {:?}", point);

                    let world_point = map_coords_to_world(point);

                    let top_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                    let top_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                    let bottom_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);
                    let bottom_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);

                    mesh_data.verts.push(&top_left);
                    mesh_data.verts.push(&top_right);
                    mesh_data.verts.push(&bottom_left);
                    mesh_data.verts.push(&bottom_right);

                    mesh_data.uvs.push(&Vector2::new(0.,0.));
                    mesh_data.uvs.push(&Vector2::new(TILE_SIZE,0.));
                    mesh_data.uvs.push(&Vector2::new(0.,TILE_SIZE));
                    mesh_data.uvs.push(&Vector2::new(TILE_SIZE,TILE_SIZE));

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

                    let bottom = map_data.get_bottom(point, world);

                    // let lowest = lowest as f32 * TILE_DIMENSIONS.y;
                    let bottom = bottom as f32 * TILE_DIMENSIONS.y;

                    // let bottom_world_pt = map_coords_to_world(bottom_point);

                    
                    //draw the sides
                    let floor_top_left = Vector3::new(
                        world_point.x, bottom, world_point.z+TILE_DIMENSIONS.z
                    );
                    let floor_top_right = Vector3::new(
                        world_point.x+TILE_DIMENSIONS.x, bottom, world_point.z+TILE_DIMENSIONS.z
                    );
                    let floor_bottom_left = Vector3::new(
                        world_point.x, bottom, world_point.z
                    );
                    let floor_bottom_right = Vector3::new(
                        world_point.x+TILE_DIMENSIONS.x, bottom, world_point.z
                    );

                    mesh_data.verts.push(&top_left);
                    mesh_data.verts.push(&top_right);
                    mesh_data.verts.push(&floor_top_left);
                    mesh_data.verts.push(&floor_top_right);

                    mesh_data.normals.push(&Vector3::new(0.,0.,1.));
                    mesh_data.normals.push(&Vector3::new(0.,0.,1.));
                    mesh_data.normals.push(&Vector3::new(0.,0.,1.));
                    mesh_data.normals.push(&Vector3::new(0.,0.,1.));

                    let top = world_point.y + TILE_DIMENSIONS.y;
                    let height = (top - bottom) * TILE_SIZE;

                    // mesh_data.uvs.push(&Vector2::new(0.,vertical_offset));
                    // mesh_data.uvs.push(&Vector2::new(TILE_SIZE,vertical_offset));
                    // mesh_data.uvs.push(&Vector2::new(0.,vertical_offset + TILE_SIZE * height));
                    // mesh_data.uvs.push(&Vector2::new(TILE_SIZE,vertical_offset + TILE_SIZE * height));

                    mesh_data.uvs.push(&Vector2::new(0.,-1.-height - bottom * TILE_SIZE));
                    mesh_data.uvs.push(&Vector2::new(TILE_SIZE,-1.-height - bottom * TILE_SIZE));
                    mesh_data.uvs.push(&Vector2::new(0.,-1.-bottom * TILE_SIZE));
                    mesh_data.uvs.push(&Vector2::new(TILE_SIZE,-1.-bottom * TILE_SIZE));

                    mesh_data.indices.push(offset);
                    mesh_data.indices.push(offset+1);
                    mesh_data.indices.push(offset+2);

                    mesh_data.indices.push(offset+2);
                    mesh_data.indices.push(offset+1);
                    mesh_data.indices.push(offset+3);

                    offset += 4;

                    // godot_print!("bottom: {:?}", (bottom, lowest));
                }
            }
        }

        for entity in changed {
            world.add_tag(entity, ManuallyChange(false)).unwrap();
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

#[derive(Copy, Clone, PartialEq)]
pub struct ManuallyChange(bool);

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
        let max = aabb.get_max() - Point::new(1,1,1);

        println!("center: {:?} dim: {:?}", aabb.center, aabb.dimensions);

        let x_min_chunk = (min.x as f32 / self.chunk_dimensions.x as f32).floor() as i32;
        let y_min_chunk = (min.y as f32 / self.chunk_dimensions.y as f32).floor() as i32;
        let z_min_chunk = (min.z as f32 / self.chunk_dimensions.z as f32).floor() as i32;

        let x_max_chunk = (max.x as f32/ self.chunk_dimensions.x as f32).floor() as i32;
        let y_max_chunk = (max.y as f32/ self.chunk_dimensions.y as f32).floor() as i32;
        let z_max_chunk = (max.z as f32/ self.chunk_dimensions.z as f32).floor() as i32;

        let mut entities: Vec<Entity> = Vec::new();

        for z in z_min_chunk..z_max_chunk+1 {
            for y in y_min_chunk..y_max_chunk+1 {
                for x in x_min_chunk..x_max_chunk+1 {

                    let pt = Point::new(x,y,z);

                    println!("Testing for chunk at {:?}", pt);

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
                                #[cfg(not(test))]
                                custom_mesh::MeshData::new(),
                            )
                        ])[0];

                        entities.push(entity);
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
                    let chunk_max = chunk_aabb.get_max() - Point::new(1,1,1);

                    let min_x = std::cmp::max(chunk_min.x, min.x);
                    let min_y = std::cmp::max(chunk_min.y, min.y);
                    let min_z = std::cmp::max(chunk_min.z, min.z);

                    let max_x = std::cmp::min(chunk_max.x, max.x);
                    let max_y = std::cmp::min(chunk_max.y, max.y);
                    let max_z = std::cmp::min(chunk_max.z, max.z);

                    println!("Range of z is {} to {}", min_z, max_z+1);
                    println!("Range of y is {} to {}", min_y, max_y+1);
                    println!("Range of x is {} to {}", min_x, max_x+1);

                    for z in min_z..max_z+1 {
                        for y in min_y..max_y+1 {
                            for x in min_x..max_x+1 {

                                let pt = Point::new(x,y,z);

                                if map_chunk.octree.insert(TileData{
                                    point: pt,
                                    ..tile_data
                                }) {
                                    println!("Inserted {:?}", pt);
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
            world.add_tag(entity, ManuallyChange(true)).unwrap();
        }

        // // TODO: This doesn't work, we need to filter it so only the edited chunks update.
        // let query = <(Write<MapChunkData>, Tagged<Point>)>::query()

        // for (mut map_chunk, chunk_pt) in query.iter_mut(&mut *world) {

        //     println!("Updating chunk {:?}", chunk_pt);

        //     let chunk_aabb = map_chunk.octree.get_aabb();
        //     let chunk_min = chunk_aabb.get_min();
        //     let chunk_max = chunk_aabb.get_max() - Point::new(1,1,1);

        //     let min_x = std::cmp::max(chunk_min.x, min.x);
        //     let min_y = std::cmp::max(chunk_min.y, min.y);
        //     let min_z = std::cmp::max(chunk_min.z, min.z);

        //     let max_x = std::cmp::min(chunk_max.x, max.x);
        //     let max_y = std::cmp::min(chunk_max.y, max.y);
        //     let max_z = std::cmp::min(chunk_max.z, max.z);

        //     for z in min_z..max_z+1 {
        //         for y in min_y..max_y+1 {
        //             for x in min_x..max_x+1 {

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


    /// Get the y coordinate where the bottom of the connected tiles sits.
    fn get_bottom(&self, point: Point, world: &legion::world::World) -> i32 {
        let chunk_bottom_y = self.octree.get_aabb().get_min().y;
        let chunk_top_y = self.octree.get_aabb().get_max().y-1;
        let mut pt = point;
        
        // If we're checking from a chunk that is higher up, bring us down to the current chunk
        if pt.y > chunk_top_y {
            pt.y = chunk_top_y;
        }

        let mut bottom: i32 = point.y;

        for y in (chunk_bottom_y-1..pt.y+1).rev() {

            pt.y = y;

            match self.octree.query_point(pt) {
                Some(_) => {
                    bottom = pt.y;  
                },
                None if y < chunk_bottom_y => {

                    let chunk_point_below = self.get_chunk_point()-Point::y();

                    let map_data_below_query = <Read<MapChunkData>>::query()
                        .filter(tag_value(&chunk_point_below));

                    match map_data_below_query.iter(world).next() {
                        Some(map_data) => {

                            let res = map_data.get_bottom(point, world); 
                            if bottom > res {
                                bottom = res;
                            }
                            break

                        },
                        None => break
                    };
                },
                None => break
            }         
        }

        bottom
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

