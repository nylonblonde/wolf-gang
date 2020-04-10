use std::collections::HashSet;
use crate::custom_mesh; 
use crate::geometry::aabb;
use crate::collections::octree::{Octree, PointData};

use std::sync::{Arc, Mutex};

use gdnative::*;

use nalgebra;

use legion::prelude::*;

use num::Integer;

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
        Point::x()+Point::z(),
        -Point::x()+Point::z(),
        -Point::x()-Point::z(),
        Point::x()-Point::z()
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

                    let world_point = map_coords_to_world(top);

                    let top_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                    let top_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                    let bottom_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);
                    let bottom_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);

                    let mut center = bottom_left + (top_right - bottom_left) / 2.;

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

                    // if there are no open sides, all we have to draw is a simple 2 triangle face
                    if open_sides.is_empty() {

                        if draw_top { 

                            mesh_data.verts.push(&top_right);
                            mesh_data.verts.push(&top_left);
                            mesh_data.verts.push(&bottom_left);
                            mesh_data.verts.push(&bottom_right);

                            mesh_data.uvs.push(&Vector2::new(TILE_SIZE,TILE_SIZE));
                            mesh_data.uvs.push(&Vector2::new(0.,TILE_SIZE));
                            mesh_data.uvs.push(&Vector2::new(0., 0.));
                            mesh_data.uvs.push(&Vector2::new(TILE_SIZE, 0.));

                            mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                            mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                            mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                            mesh_data.normals.push(&Vector3::new(0.,1.,0.));

                            mesh_data.indices.push(offset+2);
                            mesh_data.indices.push(offset);
                            mesh_data.indices.push(offset+1);

                            mesh_data.indices.push(offset+3);
                            mesh_data.indices.push(offset);
                            mesh_data.indices.push(offset+2);

                            offset += 4;
                        }
                    } else { //if open_sides is not empty, draw a more complex face to account for the bevel

                        // // for side in &open_sides {
                        // //     godot_print!("{:?}", side);
                        // // }

                        // let mut corners: Vec<Vector3> = vec![
                        //     top_right, 
                        //     top_left, 
                        //     bottom_left, 
                        //     bottom_right
                        // ];

                        // let corners_len = corners.len();

                        // let mut face_points: Vec<Vector3> = Vec::with_capacity(8);
                        // let mut curve_points: Vec<Vector3> = Vec::with_capacity(8);

                        // let mut corner_curve_points: Vec<Vector3> = Vec::with_capacity(4);
                        // let mut corner_face_indices: Vec<i32> = Vec::with_capacity(8);

                        // //cycle through the face points and get individual vectors for each side (no shared vertices)
                        // let mut bevel_points: Vec<Vector3> = Vec::with_capacity(8);
                        
                        // let mut i = 0;

                        // while i < corners_len {

                        //     let mut right = corners[i];
                        //     let next_i = (i+1) % corners_len;
                        //     let mut left = corners[next_i];

                        //     let dir = get_direction_of_edge(right, left, center);

                        //     // let next_right = corners[(i+2) % corners_len];

                        //     // let next_dir = get_direction_of_edge(left, next_right, center);

                        //     let right_dir = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_2) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                        //     let right_dir = Point::new(right_dir.x as i32, right_dir.y as i32, right_dir.z as i32);
                            
                        //     let left_dir = -right_dir;

                        //     let bevel = Vector3::new(dir.x as f32 * BEVEL_SIZE, 0., dir.z as f32 * BEVEL_SIZE);

                        //     if open_sides.contains(&dir) {
                        //         let mut scaled_right = right;
                        //         let mut scaled_left = left;
    
                        //         scaled_right -= center;
                        //         scaled_left -= center;
    
                        //         scaled_right *= 1.-BEVEL_SIZE * 2.;
                        //         scaled_left *= 1.-BEVEL_SIZE * 2.;
    
                        //         scaled_right += center;
                        //         scaled_left += center;
    
                        //         bevel_points.push(scaled_right);
                        //         bevel_points.push(scaled_left);
    
                        //         let mut right_adj = Vector3::new(0.,0.,0.);
                        //         let mut left_adj =  right_adj;
    
                        //         // let right_dir = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_2) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
    
                        //         // let right_dir = Point::new(right_dir.x as i32, right_dir.y as i32, right_dir.z as i32);
                                
                        //         if !open_sides.contains(&right_dir) {
                        //             right_adj = Vector3::new(right_dir.x as f32, 0., right_dir.z as f32) * BEVEL_SIZE;
                        //         }
    
                        //         // let left_dir = -right_dir;
    
                        //         if !open_sides.contains(&left_dir) {
                        //             left_adj = Vector3::new(left_dir.x as f32, 0., left_dir.z as f32) * BEVEL_SIZE;
                        //         }
                        //         face_points.push(scaled_right + bevel / 2. + right_adj);
                        //         face_points.push(scaled_left + bevel / 2. + left_adj);
                                
                        //     } else {

                        //         let mut scaled_right = right;
                        //         let mut scaled_left = left;

                        //         // let right_dir = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_2) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
    
                        //         // let right_dir = Point::new(right_dir.x as i32, right_dir.y as i32, right_dir.z as i32);

                        //         // let left_dir = -right_dir;

                        //         let right_diag = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_4) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                        //         let left_diag = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), -std::f32::consts::FRAC_PI_4) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);

                        //         let right_diag = Point::new(right_diag.x.round() as i32, right_diag.y as i32, right_diag.z.round() as i32);
                        //         let left_diag = Point::new(left_diag.x.round() as i32, left_diag.y as i32, left_diag.z.round() as i32);

                        //         if draw_top {
                        //             if !open_sides.contains(&right_dir) && open_sides.contains(&right_diag) {
                        //                 corner_curve_points.push(right - Vector3::new(0.,BEVEL_SIZE / 2., 0.));
                        //             }
                        //         }

                        //         // godot_print!("point {:?} dir {:?} right_diag {:?} left_diag {:?} contains right = `{:?}` contains left = `{:?}` ", point, dir, right_diag, left_diag, open_sides.contains(&right_diag), open_sides.contains(&left_diag));

                        //         let center = (right + left) / 2.;

                        //         if (open_sides.contains(&right_dir) && open_sides.contains(&left_dir)) || (open_sides.contains(&right_diag) && open_sides.contains(&left_diag)) {
                        //             scaled_right -= center;
                        //             scaled_left -= center;
        
                        //             scaled_right *= 1.-BEVEL_SIZE;
                        //             scaled_left *= 1.-BEVEL_SIZE;
        
                        //             scaled_right += center;
                        //             scaled_left += center;
                        //         } else if open_sides.contains(&right_dir) || open_sides.contains(&right_diag){
                        //             scaled_right -= left;
                        //             scaled_left -= left;

                        //             scaled_right *= 1.-BEVEL_SIZE / 2.;
                        //             scaled_left *= 1.-BEVEL_SIZE / 2.;

                        //             scaled_right += left;
                        //             scaled_left += left;
                        //         } else if open_sides.contains(&left_dir) || open_sides.contains(&left_diag) {
                        //             scaled_right -= right;
                        //             scaled_left -= right;

                        //             scaled_right *= 1.-BEVEL_SIZE / 2.;
                        //             scaled_left *= 1.-BEVEL_SIZE / 2.;

                        //             scaled_right += right;
                        //             scaled_left += right;
                        //         }

                        //         face_points.push(scaled_right);
                        //         face_points.push(scaled_left);
                        //     }
                            
                        //     i += 1;
                        // }

                        // let mut border_points: Vec<Vector3> = Vec::with_capacity(24);

                        // //cycle through the bevel points and make any adjustments needed to deal with adjacent tiles
                        // let mut border_points_tentative: Vec<Vector3> = Vec::with_capacity(8);
                        // let mut i = 0;
                        // while i < bevel_points.len() {

                        //     let mut right = bevel_points[i];
                        //     let mut left = bevel_points[i+1];

                        //     let dir = get_direction_of_edge(left, right, center);
                        //     let bevel = Vector3::new(dir.x as f32 * BEVEL_SIZE, 0., dir.z as f32 * BEVEL_SIZE);

                        //     let mut right_adj: Option<Vector3> = None;
                        //     let mut left_adj: Option<Vector3> = None;

                        //     let right_dir = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_2) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);

                        //     let right_dir = Point::new(right_dir.x as i32, right_dir.y as i32, right_dir.z as i32);
                            
                        //     if !open_sides.contains(&right_dir) {
                        //         right_adj = Some(Vector3::new(right_dir.x as f32, 0., right_dir.z as f32) * BEVEL_SIZE);
                        //     }

                        //     let left_dir = -right_dir;

                        //     if !open_sides.contains(&left_dir) {
                        //         left_adj = Some(Vector3::new(left_dir.x as f32, 0., left_dir.z as f32) * BEVEL_SIZE);
                        //     }

                        //     let mut y_adj = Vector3::new(0.,0.,0.);

                        //     let right_diag = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_4) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                        //     let right_diag = Point::new(right_diag.x.round() as i32, right_diag.y as i32, right_diag.z.round() as i32);

                        //     if open_sides.contains(&dir) {

                        //         if draw_top {
                        //             y_adj.y = BEVEL_SIZE / 2.;
                        //         }

                        //         if let Some(left_adj) = left_adj {
                        //             left += left_adj;
                        //         }

                        //         if let Some(right_adj) = right_adj {
                        //             right += right_adj;
                        //         }

                        //         border_points_tentative.push(right + bevel - y_adj);
                        //         border_points_tentative.push(left + bevel - y_adj);

                        //         curve_points.push(right + bevel - y_adj);
                        //         curve_points.push(left + bevel - y_adj);

                        //     }

                        //     i += 2;
                        // }

                        // if draw_top {

                        //     let mut face_points_final: Vec<Vector3> = Vec::with_capacity(8);
                        //     let face_points_len = face_points.len();

                        //     mesh_data.verts.push(&center);
                        //     mesh_data.uvs.push(&Vector2::new(TILE_SIZE / 2., TILE_SIZE / 2.));
                        //     mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                        //     offset += 1;

                        //     let mut i = 0;
                        //     while i < face_points_len {
                                
                        //         let right = face_points[i % face_points_len];
                        //         let next_i = (i + 1) % face_points_len;
                        //         let left = face_points[next_i];

                        //         // let dir = get_direction_of_edge(right, left, center);

                        //         if (left - right).length() > std::f32::EPSILON {

                        //             face_points_final.push(right);

                        //         }
                        //         i += 1;
                        //     }

                        //     let begin = offset;
                        //     let face_points_final_len = face_points_final.len();

                        //     let mut curves_iter = curve_points.clone().into_iter();
                        //     let mut corner_curves_iter = corner_curve_points.clone().into_iter();

                        //     let mut face_indices: Vec<i32> = Vec::with_capacity(16);

                        //     godot_print!("curve and face lens {:?} {:?}", curve_points.len(), face_points_final_len);

                        //     let mut i = 0;
                        //     let mut j = 0;
                        //     while i < face_points_final_len {

                        //         let right = face_points_final[i % face_points_final_len];
                        //         let next_i = (i + 1) % face_points_final_len;
                        //         let left = face_points_final[next_i];

                        //         let dir = get_direction_of_edge(right, left, center);

                        //         let u = (right.x - world_point.x).abs() * TILE_SIZE;
                        //         let v = (right.z - world_point.z).abs() * TILE_SIZE;

                        //         mesh_data.verts.push(&right);
                        //         mesh_data.uvs.push(&Vector2::new(u, v));
                        //         mesh_data.normals.push(&Vector3::new(0.,1.,0.));

                        //         if i < face_points_final_len - 1 {
                        //             mesh_data.indices.push(begin);
                        //             mesh_data.indices.push(begin + i as i32);
                        //             mesh_data.indices.push(begin + (i as i32 + 1) % face_points_final_len as i32);
                        //         }

                        //         let right_diag = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_4) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                        //         let right_diag = Point::new(right_diag.x.round() as i32, right_diag.y as i32, right_diag.z.round() as i32);

                        //         if open_sides.contains(&dir) {
                        //             // godot_print!("index = {:?} face_pt = {:?} curve = {:?}", begin + i as i32, right, curves_iter.clone().next().unwrap());
                                    

                        //             if let Some(_) = curves_iter.next() {
                                        
                        //                 face_indices.push(begin + i as i32);

                        //                 // if let None = curves_iter.clone().next() {
                        //                 //     face_indices.push(begin + (i as i32 + 1) % face_points_final_len as i32);
                        //                 // }

                        //                 //if the next direction isn't open, we have to add the left index as well so the whole side gets defined
                                        
                        //                 let left_dir = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), -std::f32::consts::FRAC_PI_2) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                        //                 let left_dir = Point::new(left_dir.x as i32, left_dir.y as i32, left_dir.z as i32);

                        //                 // let next_dir = get_direction_of_edge(next_right, next_left, center);

                        //                 if j % 2 == 0 {
                        //                     if !open_sides.contains(&left_dir) {
                        //                         godot_print!("this happens at j {:?}", j);
                        //                         face_indices.push(begin + (i as i32 + 1) % face_points_final_len as i32);
                        //                     }
                        //                 }
                                        
                        //                 //j helps us determine whether we're on the right pt or left pt of the side.
                        //                 j += 1;

                        //             }

                        //         } else if open_sides.contains(&right_diag) {

                        //             // godot_print!("{:?}", corner_curve_points.len());

                        //             //no need to check if open sides contains right_dir since corner_curve_points only gets pushed to in that case anyway
                        //             if let Some(r) = corner_curves_iter.next() {
                        //                 let next_right = face_points_final[(i + 2) % face_points_final_len];

                        //                 corner_face_indices.push(begin + (i as i32 + 1) % face_points_final_len as i32);
                        //                 corner_face_indices.push(begin + (i as i32 + 2) % face_points_final_len as i32);

                        //                 godot_print!("corner_curve pt {:?} {:?} {:?}", r, left, next_right);

                        //             }

                        //         }

                        //         offset += 1;
                        //         i += 1;
                        //     }

                        //     let curve_points_len = curve_points.len();
                        //     let mut i = 0;
                        //     let begin = offset;

                        //     godot_print!("lengths {:?} {:?}", curve_points_len, face_indices.len());

                        //     while i < curve_points_len {

                        //         let right = curve_points[i];
                        //         let left = curve_points[(i + 1) & curve_points_len];
                        //         let right_face = face_indices[i];
                        //         let left_face = face_indices[(i + 1) % face_indices.len()];

                        //         godot_print!("{:?} {:?}", right_face, left_face);
                                
                        //         let u = (right.x - world_point.x).abs() * TILE_SIZE;
                        //         let v = (right.z - world_point.z).abs() * TILE_SIZE;

                        //         mesh_data.verts.push(&right);
                        //         mesh_data.uvs.push(&Vector2::new(u,v));
                        //         mesh_data.normals.push(&Vector3::new(0.,1.,0.));

                                
                        //         if (left - right).length() >=  1. {
                        //         // if i < curve_points_len - 1 {
                        //             mesh_data.indices.push(left_face);
                        //             // godot_print!("{:?}", left_face);
                        //             mesh_data.indices.push(right_face);
                        //             // godot_print!("{:?}", right_face);
                        //             mesh_data.indices.push(begin + i as i32);
                        //             // godot_print!("{:?}", begin + i as i32);
                                
                        //             mesh_data.indices.push(begin + (i as i32 + 1) % curve_points_len as i32);
                        //             // godot_print!("{:?}", begin + (i as i32 + 1) % curve_points_len as i32);
                        //             mesh_data.indices.push(left_face);
                        //             // godot_print!("{:?}", left_face);
                        //             mesh_data.indices.push(begin + i as i32);
                        //             // godot_print!("{:?}", begin + i as i32);
                        //         }

                        //         i += 2;
                        //     }

                        //     offset += i as i32;

                        // }

                        // // godot_print!("{:?} {:?}", corner_curve_points.len(), corner_face_indices.len());

                        // // let corner_curve_points_len = corner_curve_points.len();
                        // // let mut corner_face_indices_iter = corner_face_indices.into_iter();
                        
                        // // for corner_curve_point in corner_curve_points {

                        // //     // godot_print!("drawing corner point {:?}", corner_curve_point);

                        // //     mesh_data.verts.push(&corner_curve_point);

                        // //     let u = (corner_curve_point.x - world_point.x).abs() * TILE_SIZE;
                        // //     let v = (corner_curve_point.z - world_point.z).abs() * TILE_SIZE;

                        // //     mesh_data.uvs.push(&Vector2::new(u,v));
                        // //     mesh_data.normals.push(&Vector3::new(0.,1.,0.));

                        // //     let right_face = corner_face_indices_iter.next().unwrap();
                        // //     let left_face = corner_face_indices_iter.next().unwrap();

                        // //     mesh_data.indices.push(left_face);
                        // //     mesh_data.indices.push(right_face);
                        // //     mesh_data.indices.push(offset);

                        // //     offset += 1;

                        // // }

                        // //finalize the border points, adding corner verts for the beveled corners
                        // let border_points_tentative_len = border_points_tentative.len();
                        // let mut i = 0; 
                        // while i < border_points_tentative_len {

                        //     let left = border_points_tentative[i];

                        //     let right = border_points_tentative[i+1];

                        //     border_points.push(left);
                        //     border_points.push(right);

                        //     let next_i = (i + 2) % border_points_tentative_len;

                        //     let next_left = border_points_tentative[next_i];

                        //     let distance = (next_left - right).length();

                        //     //check the distance to make sure it's actually an adjacent corner, since the "next left" could loop around to be on the other side on corner tiles where not all sides are drawn
                        //     if distance < 1. {

                        //         let average = (right + next_left) / 2.;

                        //         //only put in a corner point if the corners aren't touching. Need two to not get a flipped uv
                        //         if (right - average).length() > std::f32::EPSILON {
                        //             border_points.push(right);
                        //             border_points.push(average);
                        //             border_points.push(average);
                        //             border_points.push(next_left);
                        //         }
                        //     }

                        //     i += 2;
                        // }

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

                                    // panic!("{:?}", (i as i32 - 1).mod_floor(&(border_points_len as i32)));

                                    let diff = *next_point - *border_point;

                                    let mut u = 1.;
                                    let mut next_u = 1.;

                                    if dir.z.abs() > 0 {

                                        u = world_point.x * TILE_SIZE + (border_point.x - world_point.x).abs() * TILE_SIZE;

                                        next_u = world_point.x * TILE_SIZE + (next_point.x - world_point.x).abs() * TILE_SIZE ;

                                        if diff.x > 0. {

                                            u = -u;
                                            next_u = -next_u;

                                        }

                                    } else if dir.x.abs() > 0 {
                                        u = world_point.z * TILE_SIZE + (border_point.z - world_point.z).abs() * TILE_SIZE;

                                        next_u = world_point.z * TILE_SIZE + (next_point.z - world_point.z).abs() * TILE_SIZE ;

                                        if diff.z > 0. {

                                            u = -u;
                                            next_u = -next_u;

                                        }
                                    }
                                    mesh_data.uvs.push(&Vector2::new(u,-1.-height * TILE_SIZE - bottom * TILE_SIZE));
                                    mesh_data.uvs.push(&Vector2::new(u,-1.-bottom * TILE_SIZE));

                                    mesh_data.uvs.push(&Vector2::new(next_u,-1.-height * TILE_SIZE - bottom * TILE_SIZE));
                                    mesh_data.uvs.push(&Vector2::new(next_u,-1.-bottom * TILE_SIZE));

                                } 

                                //if there are only 2 border points, only draw from the first index to avoid drawing both sides since the index will loop around
                                if border_points_len > 2 || i < border_points_len-1 {
                                    //only add indices for points that aren't overlapping
                                    if (*next_point - *border_point).length() > std::f32::EPSILON {

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
                                    } else {
                                        // godot_print!("Skipped some points because they were too close");
                                    }
                                }

                                offset += 2;

                            }
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

pub fn get_open_sides(neighbor_dirs: &[Point; 8], world: &legion::world::World, map_data: &MapChunkData, point: Point, checked: &HashSet<Point>) -> HashSet<Point> {
    let mut open_sides: HashSet<Point> = HashSet::new();
    let chunk_max = map_data.octree.get_aabb().get_max();
    let chunk_min = map_data.octree.get_aabb().get_min();
    
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

                        // if the dir is pointing diagonally, and this point is not on the corner, skip this step
                        if dir.x.abs() + dir.z.abs() > 1 &&
                            (*dir == Point::new(1,0,1) && !(point.x == chunk_max.x && point.z == chunk_max.z)) ||
                            (*dir == Point::new(-1,0,1) && !(point.x == chunk_min.x && point.z == chunk_max.z)) ||
                            (*dir == Point::new(-1,0,-1) && !(point.x == chunk_min.x && point.z == chunk_min.z)) ||
                            (*dir == Point::new(1,0,-1) && !(point.x == chunk_max.x && point.z == chunk_min.z)) {
                                continue;
                        }

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