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
type Vector3D = nalgebra::Vector3<f32>;

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
pub const BEVEL_SIZE: f32 = 0.1;

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

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ChangeType {
    Direct,
    Indirect
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct ManuallyChange(ChangeType);

pub fn create_drawing_thread_local_fn() -> Box<dyn FnMut(&mut legion::world::World, &mut Resources)>  {
    let write_mesh_query = <(Read<MapChunkData>, Write<custom_mesh::MeshData>, Tagged<ManuallyChange>)>::query();
    
    let neighbor_dirs = [
        Point::x(),
        -Point::x(),
        Point::z(),
        -Point::z(),
        Point::x()+Point::z(),
        -Point::x()+Point::z(),
        -Point::x()-Point::z(),
        Point::x()+Point::z()
    ];

    Box::new(move |world, _| {

        unsafe {

            //for entities that need to have ManuallyChange added
            let mut to_change: HashSet<Entity> = HashSet::new();

            //for entities that need to have ManuallyChange removed
            let mut to_changed: HashSet<Entity> = HashSet::new();

            for (entity, (map_data, mut mesh_data, changed)) in write_mesh_query.iter_entities_unchecked(world) {

                to_changed.insert(entity);

                //only manually change neighbors if it is a direct change
                if changed.0 == ChangeType::Direct {
                    for dir in &neighbor_dirs {
                        
                        let neighbor_chunk_pt = map_data.get_chunk_point() + dir;

                        let neighbor_chunk_query = <Read<MapChunkData>>::query()
                            .filter(tag_value(&neighbor_chunk_pt));

                        for (entity, _) in neighbor_chunk_query.iter_entities_unchecked(world) {
                            to_change.insert(entity);
                        }
                    }
                }

                godot_print!("Drawing {:?}", map_data.get_chunk_point());
                mesh_data.verts = Vector3Array::new();
                mesh_data.normals = Vector3Array::new();
                mesh_data.uvs = Vector2Array::new();
                mesh_data.indices = Int32Array::new();

                let mut checked: HashSet::<Point> = HashSet::new();

                let mut offset = 0;
                for tile in map_data.octree.clone().into_iter() {

                    let point = tile.get_point();

                    if checked.contains(&point) {
                        continue
                    }

                    checked.insert(point);

                    let mut top = point;
                    let mut draw_top: bool = true;
                    let chunk_top_y = map_data.octree.get_aabb().get_max().y;

                    //iterate from this point to either the top or the top of the chunk
                    for y in point.y..chunk_top_y+1 {
                        top.y = y;

                        let point_above = top+Point::y();
                        checked.insert(point_above);

                        match map_data.octree.query_point(point_above) {
                            Some(_) => continue,
                            None if y == chunk_top_y => {
                                
                                let chunk_point_above = map_data.get_chunk_point()+Point::y();

                                let chunk_point_above_query = <Read<MapChunkData>>::query()
                                    .filter(tag_value(&chunk_point_above));

                                match chunk_point_above_query.iter(world).next() {
                                    Some(map_data) => {
                                        match map_data.octree.query_point(point_above) {
                                            Some(_) => { 
                                                draw_top = false; 
                                                break
                                            },
                                            None => break
                                        }

                                    },
                                    None => break
                                }

                            },
                            None => break
                        }
                    }

                    let mut open_sides: HashSet<Point> = HashSet::new();

                    for dir in &neighbor_dirs {

                        let neighbor = top + dir;

                        match map_data.octree.query_point(neighbor) {
                            Some(_) => continue,
                            None => {

                                match map_data.octree.get_aabb().contains_point(neighbor) {
                                    false => {
                                        let chunk_point_dir = map_data.get_chunk_point() + dir;

                                        let chunk_point_dir_query = <Read<MapChunkData>>::query()
                                            .filter(tag_value(&chunk_point_dir));

                                        match chunk_point_dir_query.iter(world).next() {
                                            Some(map_data) => {
                                                
                                                match map_data.octree.query_point(neighbor) {
                                                    Some(_) => continue,
                                                    None => {
                                                        open_sides.insert(*dir);
                                                    }
                                                }

                                            },
                                            None => { open_sides.insert(*dir); }
                                        }
                                    },
                                    true => { open_sides.insert(*dir); }
                                }
                            }
                        }
                    }

                    let mut border_points: Vec<Vector3> = Vec::new();
                    let mut face_points: Vec<Vector3> = Vec::new();

                    let world_point = map_coords_to_world(top);
                    let center = (Vector3::new(world_point.x, 0., world_point.z) * 2. + Vector3::new(TILE_DIMENSIONS.x as f32, 0., TILE_DIMENSIONS.z as f32)) / 2.;

                    let top_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                    let top_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                    let bottom_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);
                    let bottom_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);

                    //needs to be added in clockwise or counterclockwise order
                    face_points.push(top_right);
                    face_points.push(top_left);
                    face_points.push(bottom_left);
                    face_points.push(bottom_right);

                    let face_points_len = face_points.len();

                    for i in 0..face_points_len {

                        let face_point = face_points[i];
                        let next_i = (i+1) % face_points_len;
                        let next_point = face_points[next_i];

                        let dir = get_direction_of_edge(face_point, next_point, center);

                        if open_sides.contains(&dir) {
                            let bevel = Vector3::new(dir.x as f32 * BEVEL_SIZE, dir.y as f32 * BEVEL_SIZE, dir.z as f32 * BEVEL_SIZE);
                            face_points[i] = face_point - bevel;
                            face_points[next_i] = next_point - bevel;
                        }

                        if i > 0 {
                            border_points.push(face_points[i]);

                            if i == face_points_len - 1 {
                                border_points.push(face_points[next_i]);
                            }
                        }
                    }

                    if draw_top { 
                        mesh_data.verts.push(&face_points[1]);
                        mesh_data.verts.push(&face_points[0]);
                        mesh_data.verts.push(&face_points[2]);
                        mesh_data.verts.push(&face_points[3]);

                        let left_offset = (face_points[1].x - top_left.x) * TILE_SIZE;
                        let right_offset = (top_right.x - face_points[0].x) * TILE_SIZE;

                        let top_offset = (top_left.z - face_points[1].z) * TILE_SIZE;
                        let bottom_offset = (face_points[2].z - bottom_left.z) * TILE_SIZE;

                        mesh_data.uvs.push(&Vector2::new(left_offset,top_offset));
                        mesh_data.uvs.push(&Vector2::new(TILE_SIZE - right_offset,top_offset));
                        mesh_data.uvs.push(&Vector2::new(left_offset,TILE_SIZE - bottom_offset));
                        mesh_data.uvs.push(&Vector2::new(TILE_SIZE - right_offset,TILE_SIZE - bottom_offset));

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

                    let mut bottom = point;
                    let chunk_bottom_y = map_data.octree.get_aabb().get_min().y;
                    
                    let mut draw_bottom: bool = true;

                    for y in (chunk_bottom_y-1..point.y+1).rev() {
                        bottom.y = y;

                        let point_below = bottom - Point::y();
                        
                        checked.insert(point_below);

                        match map_data.octree.query_point(point_below) {
                            Some(_) => continue,
                            None if y == chunk_bottom_y => {
                                let chunk_point_below = map_data.get_chunk_point() - Point::y();
                                
                                let chunk_point_below_query = <Read<MapChunkData>>::query()
                                    .filter(tag_value(&chunk_point_below));

                                match chunk_point_below_query.iter(world).next() {

                                    Some(map_data) => {
                                        match map_data.octree.query_point(point_below) {
                                            Some(_) => {
                                                draw_bottom = false;
                                                break
                                            },
                                            None => break
                                        }
                                    },
                                    None => break
                                }
                            },
                            None => break
                        }
                        
                    }

                    let bottom = map_coords_to_world(bottom).y;

                    let top = world_point.y + TILE_DIMENSIONS.y;
                    let height = top - bottom;

                    let border_points_len = border_points.len();
                    let begin = offset;
                    let indices_len = border_points_len as i32 * 2;
                    let mut index_count = offset;

                    //define the sides
                    for i in 0..border_points_len {
                        
                        let border_point = border_points.get(i).unwrap();

                        //get the direction
                        let next_i = (i+1) % border_points_len;
                        let next_point = border_points.get(next_i).unwrap();

                        let dir = get_direction_of_edge(*border_point, *next_point, center);
                        // godot_print!("average_dir = {:?} dir = {:?}", average_dir, dir);

                        //top
                        mesh_data.verts.push(&border_point);

                        //bottom
                        mesh_data.verts.push(&(*border_point - Vector3::new(0., height, 0.)));

                        mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                        mesh_data.normals.push(&Vector3::new(0.,1.,0.));

                        let uv_width = 1.; //num::Float::max(top_right.x - top_left.x, top_right.z - top_left.z);

                        if i % 2 == 0 {
                            mesh_data.uvs.push(&Vector2::new(0.,-1.-height * TILE_SIZE - bottom * TILE_SIZE));
                            mesh_data.uvs.push(&Vector2::new(0.,-1.-bottom * TILE_SIZE));
                        } else {
                            mesh_data.uvs.push(&Vector2::new(TILE_SIZE * uv_width,-1.-height * TILE_SIZE - bottom * TILE_SIZE));
                            mesh_data.uvs.push(&Vector2::new(TILE_SIZE * uv_width,-1.-bottom * TILE_SIZE));
                        }

                        if open_sides.contains(&dir) {

                            let j = offset - begin;

                            mesh_data.indices.push(j % indices_len + begin);
                            mesh_data.indices.push((j+1) % indices_len + begin);
                            mesh_data.indices.push((j+2) % indices_len + begin);

                            mesh_data.indices.push((j+2) % indices_len + begin);
                            mesh_data.indices.push((j+1) % indices_len + begin);
                            mesh_data.indices.push((j+3) % indices_len + begin);

                        } else {
                            // godot_print!("{:?} is not drawing {:?}", point, dir);
                        }

                        offset += 2;

                    }
                }
            }

            for entity in to_change {
                world.add_tag(entity, ManuallyChange(ChangeType::Indirect)).unwrap();
            }

            for entity in to_changed {
                world.remove_tag::<ManuallyChange>(entity).unwrap();
            }
        }

    })
} 

/// Get the direction the average of two points are from the center. For calculating the orthogonal direction of edges.
pub fn get_direction_of_edge(pt1: Vector3, pt2: Vector3, center: Vector3) -> Point {
    let right_dir = Vector3::new(1.,0.,0.);
    let forward_dir = Vector3::new(0.,0.,1.);
    let back_dir = -forward_dir;
    let left_dir = -right_dir;

    let mut average = (pt1 + pt2) / 2.;
    average.y = 0.;

    let average_dir = (average - center).normalize();

    let dir = std::cmp::max_by(forward_dir, 
        std::cmp::max_by(left_dir, 
            std::cmp::max_by(back_dir, 
                right_dir, |lh, rh|{
                    // godot_print!("lh = {:?} rh = {:?} lh dot = {:?} rh dot = {:?}", lh, rh, lh.dot(average_dir), rh.dot(average_dir));
                    lh.dot(average_dir).partial_cmp(&rh.dot(average_dir)).unwrap()
                }), 
            |lh, rh| {
                // godot_print!("lh = {:?} rh = {:?} lh dot = {:?} rh dot = {:?}", lh, rh, lh.dot(average_dir), rh.dot(average_dir));

                lh.dot(average_dir).partial_cmp(&rh.dot(average_dir)).unwrap()
            }), 
        |lh, rh| {
            // godot_print!("lh = {:?} rh = {:?} lh dot = {:?} rh dot = {:?}", lh, rh, lh.dot(average_dir), rh.dot(average_dir));

            lh.dot(average_dir).partial_cmp(&rh.dot(average_dir)).unwrap()
        }
    );

    Point::new(dir.x as i32, dir.y as i32, dir.z as i32)
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
                        #[cfg(not(test))]
                        custom_mesh::MeshData::new(),
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
            world.add_component(entity, map_chunk).unwrap();
            world.add_tag(entity, ManuallyChange(ChangeType::Direct)).unwrap();
        }
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

#[derive(Clone, Debug)]
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