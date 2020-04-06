use std::collections::HashSet;
use crate::custom_mesh; 
use crate::geometry::aabb;
use crate::collections::octree::{Octree, PointData};

use std::sync::{Arc, Mutex};

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
pub const BEVEL_SIZE: f32 = 0.2;

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

#[derive(Clone, Debug, Default)]
struct OpenSides {
    sides_map: HashMap<Point, HashSet<Point>>
}

pub fn create_side_check_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("side_check_system")
        .with_query(<(Read<MapChunkData>, Write<OpenSides>, Tagged<ManuallyChange>)>::query()
        )
        .build(move |commands, world, resources, query| {

            query.par_for_each_mut(&mut *world, move |(map_data, open_sides, changed)|{
                println!("checking {:?} {:?}", map_data.get_chunk_point(), changed.0);

                
            });

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
        // Point::x()+Point::z(),
        // -Point::x()+Point::z(),
        // -Point::x()-Point::z(),
        // Point::x()+Point::z()
    ];

    Box::new(move |world, _| {

        unsafe {

            //for entities that need to have ManuallyChange added
            let to_change: Arc<Mutex<HashSet<Entity>>> = Arc::new(Mutex::new(HashSet::new()));

            //for entities that need to have ManuallyChange removed
            let to_changed: Arc<Mutex<HashSet<Entity>>> = Arc::new(Mutex::new(HashSet::new()));

            write_mesh_query.par_entities_for_each_unchecked(world, |(entity, (map_data, mut mesh_data, changed))|{

                {
                    let mut to_changed = to_changed.lock().unwrap();
                    to_changed.insert(entity);
                }

                //only manually change neighbors if it is a direct change
                if changed.0 == ChangeType::Direct {
                    for dir in &neighbor_dirs {
                        
                        let neighbor_chunk_pt = map_data.get_chunk_point() + dir;

                        let neighbor_chunk_query = <Read<MapChunkData>>::query()
                            .filter(tag_value(&neighbor_chunk_pt));

                        for (entity, _) in neighbor_chunk_query.iter_entities_unchecked(world) {
                            
                            let mut to_change = to_change.lock().unwrap();
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
                    
                    let point_sides = get_open_sides(&neighbor_dirs, world, &map_data, point, &checked);

                    //iterate from this point to either the top or the top of the chunk
                    for y in point.y..chunk_top_y+1 {
                        top.y = y;

                        let point_above = top+Point::y();

                        match map_data.octree.query_point(point_above) {
                            Some(_) => {
                                let curr_sides = get_open_sides(&neighbor_dirs, world, &map_data, point_above, &checked);

                                if curr_sides.symmetric_difference(&point_sides).count() > 0 {
                                    draw_top = false;
                                    break;
                                }

                                checked.insert(point_above);
                            },
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

                    // let open_sides = get_open_sides(&neighbor_dirs, world, &map_data, top);

                    let open_sides = get_open_sides(&neighbor_dirs, world, &map_data, top, &checked);

                    let mut border_points: Vec<Vector3> = Vec::with_capacity(12);

                    let world_point = map_coords_to_world(top);
                    let mut center = (Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z) * 2. + Vector3::new(TILE_DIMENSIONS.x as f32, world_point.y+TILE_DIMENSIONS.y, TILE_DIMENSIONS.z as f32)) / 2.;

                    let top_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                    let top_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                    let bottom_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);
                    let bottom_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);

                    // if there are no open sides, all we have to draw is a simple 2 triangle face
                    if open_sides.is_empty() {

                        if draw_top { 

                            mesh_data.verts.push(&top_left);
                            mesh_data.verts.push(&top_right);
                            mesh_data.verts.push(&bottom_left);
                            mesh_data.verts.push(&bottom_right);

                            mesh_data.uvs.push(&Vector2::new(0.,0.));
                            mesh_data.uvs.push(&Vector2::new(TILE_SIZE,0.));
                            mesh_data.uvs.push(&Vector2::new(0.,TILE_SIZE));
                            mesh_data.uvs.push(&Vector2::new(TILE_SIZE ,TILE_SIZE));

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
                    } else { //if open_sides is not empty, draw a more complex face to account for the bevel

                        let face_points: Vec<Vector3> = vec![
                            top_right + Vector3::new(-BEVEL_SIZE, 0., -BEVEL_SIZE), 
                            top_left + Vector3::new(BEVEL_SIZE, 0., -BEVEL_SIZE), 
                            bottom_left + Vector3::new(BEVEL_SIZE, 0., BEVEL_SIZE), 
                            bottom_right + Vector3::new(-BEVEL_SIZE, 0., BEVEL_SIZE)
                        ];

                        let face_points_len = face_points.len();

                        let mut bevel_points: Vec<Vector3> = Vec::with_capacity(12);

                        for i in 0..face_points_len {
                            let face_point = face_points[i];
                            let next_i = (i+1) % face_points_len;
                            let next_point = face_points[next_i];

                            let dir = get_direction_of_edge(face_point, next_point, center);

                            if open_sides.contains(&dir) {
                                let bevel = Vector3::new(dir.x as f32 * BEVEL_SIZE, 0., dir.z as f32 * BEVEL_SIZE);

                                bevel_points.push(face_point + bevel);
                                bevel_points.push(next_point + bevel);
                            }
                        }

                        let mut i = 0;
                        while i < bevel_points.len() {

                            let mut left = bevel_points[i];
                            let mut right = bevel_points[i+1];

                            let dir = get_direction_of_edge(left, right, center);

                            if open_sides.contains(&dir) {

                                let side_dir = if dir.x > 0 {
                                    Point::new(0,0,1)
                                } else if dir.z > 0 {
                                    Point::new(-1,0,0)
                                } else if dir.x < 0 {
                                    Point::new(0,0,-1)
                                } else {
                                    Point::new(1,0,0)
                                };

                                if !open_sides.contains(&side_dir) {
                                    right += Vector3::new(side_dir.x as f32, 0., side_dir.z as f32) * BEVEL_SIZE;
                                }

                                let side_dir = -side_dir;

                                if !open_sides.contains(&side_dir) {
                                    left += Vector3::new(side_dir.x as f32, 0., side_dir.z as f32) * BEVEL_SIZE;
                                }

                                if draw_top {
                                    left.y -= BEVEL_SIZE / 2.;
                                    right.y -= BEVEL_SIZE / 2.;
                                }

                                border_points.push(left);
                                border_points.push(right);
                            } 

                            i += 2;
                        }

                    }

                    let mut bottom = point;
                    let chunk_bottom_y = map_data.octree.get_aabb().get_min().y;
                    
                    let mut draw_bottom: bool = true;

                    for y in (chunk_bottom_y-1..point.y+1).rev() {
                        bottom.y = y;

                        let point_below = bottom - Point::y();
                        
                        match map_data.octree.query_point(point_below) {
                            Some(_) => {
                                let curr_sides = get_open_sides(&neighbor_dirs, world, &map_data, point_below, &checked);

                                if curr_sides.symmetric_difference(&point_sides).count() > 0 {
                                    break;
                                }

                                checked.insert(point_below);
                            },
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

                    let border_points_len = border_points.len();

                    if border_points_len > 0 {
                        let bottom = map_coords_to_world(bottom).y;

                        let top = border_points[0].y;
                        let height = top - bottom;

                        let begin = offset;
                        let indices_len = border_points_len as i32 * 2;

                        center.y = 0.;

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
            });

            let to_change = to_change.lock().unwrap();
            let to_changed = to_changed.lock().unwrap();

            for entity in &*to_change {
                world.add_tag(*entity, ManuallyChange(ChangeType::Indirect)).unwrap();
            }

            for entity in &*to_changed {
                world.remove_tag::<ManuallyChange>(*entity).unwrap();
            }
        }

    })
} 

pub fn get_open_sides(neighbor_dirs: &[Point; 4], world: &legion::world::World, map_data: &MapChunkData, point: Point, checked: &HashSet<Point>) -> HashSet<Point> {
    let mut open_sides: HashSet<Point> = HashSet::new();
    // let chunk_max = map_data.octree.get_aabb().get_max();
    // let chunk_min = map_data.octree.get_aabb().get_min();
    
    for dir in neighbor_dirs {

        let neighbor = point + *dir;

        if checked.contains(&neighbor) {
            continue;
        }

        match map_data.octree.query_point(neighbor) {
            Some(_) => continue,
            None => {

                match map_data.octree.get_aabb().contains_point(neighbor) {
                    false => {

                        // // if the dir is pointing diagonally, and this point is not on the corner, skip this step
                        // if dir.x.abs() + dir.z.abs() > 1 &&
                        //     (*dir == Point::new(1,0,1) && !(point.x == chunk_max.x && point.z == chunk_max.z)) ||
                        //     (*dir == Point::new(-1,0,1) && !(point.x == chunk_min.x && point.z == chunk_max.z)) ||
                        //     (*dir == Point::new(-1,0,-1) && !(point.x == chunk_min.x && point.z == chunk_min.z)) ||
                        //     (*dir == Point::new(1,0,-1) && !(point.x == chunk_max.x && point.z == chunk_min.z)) {
                        //         continue;
                        // }

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
    open_sides
}

/// Get the direction the average of two points are from the center. For calculating the orthogonal direction of edges.
fn get_direction_of_edge(pt1: Vector3, pt2: Vector3, center: Vector3) -> Point {
    let right_dir = Vector3::new(1.,0.,0.);
    let forward_dir = Vector3::new(0.,0.,1.);
    let back_dir = -forward_dir;
    let left_dir = -right_dir;

    let mut average = (pt1 + pt2) / 2.;
    average.y = 0.;

    let heading = average - center;

    //because normalize returns NaN if the distance btwn points is zero
    if heading.length() < std::f32::EPSILON {
        return Point::zeros();
    }

    let average_dir = heading.normalize();

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
                        OpenSides::default(),
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