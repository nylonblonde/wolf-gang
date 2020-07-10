/// Handles the creation and defining of mesh nodes in Godot

use std::collections::HashSet;
use crate::systems::custom_mesh; 
use crate::geometry::aabb;
use crate::collections::octree::PointData;

use std::sync::{Arc, Mutex};

use gdnative::*;

use nalgebra;

use legion::prelude::*;

type AABB = aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;
type Vector3D = nalgebra::Vector3<f32>;

use rayon::prelude::*;
use std::sync::mpsc;

use super::*;

pub const TILE_PIXELS: f32 = 64.;
pub const SHEET_PIXELS: f32 = 1024.;
pub const TILE_SIZE: f32 = TILE_PIXELS/SHEET_PIXELS;
pub const BEVEL_SIZE: f32 = 0.2;
pub const START_REPEAT_ABOVE_HEIGHT: f32 = 7.;
pub const START_REPEAT_BELOW_HEIGHT: f32 = 0.;
pub const REPEAT_AMOUNT_ABOVE: f32 = 4.;
pub const REPEAT_AMOUNT_BELOW: f32 = 4.;

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
    let write_mesh_query = <(Read<MapChunkData>, Write<custom_mesh::MeshData>, Tagged<ManuallyChange>)>::query();
    let map_query = <(Read<MapChunkData>, Tagged<ManuallyChange>)>::query();
    
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

    let all_dirs = [
        Point::x(),
        -Point::x(),
        Point::z(),
        -Point::z(),
        Point::x()+Point::z(),
        -Point::x()+Point::z(),
        -Point::x()-Point::z(),
        Point::x()-Point::z(),
        Point::y(),
        -Point::y()
    ];

    Box::new(move |world, _| {
        
        unsafe {

            // for (entity, (map_data, mut mesh_data, changed)) in write_mesh_query.iter_entities_unchecked(world) { 
            write_mesh_query.par_for_each_unchecked(world, |(map_data, mut mesh_data, _)|{

                godot_print!("Drawing {:?}", map_data.get_chunk_point());
                mesh_data.verts = Vector3Array::new();
                mesh_data.normals = Vector3Array::new();
                mesh_data.uvs = Vector2Array::new();
                mesh_data.uv2s = Vector2Array::new();
                mesh_data.indices = Int32Array::new();

                let mut checked: HashSet::<Point> = HashSet::new();

                let mut offset = 0;
                for tile in map_data.octree.clone().into_iter() {

                    let point = tile.get_point();

                    if checked.contains(&point) {
                        continue
                    }

                    checked.insert(point);

                    let chunk_top_y = map_data.octree.get_aabb().get_max().y;

                    let true_top = get_true_top(point, world, &map_data, &checked);
                    let true_top = map_coords_to_world(true_top).y;

                    let mut top = point;
                    let mut draw_top: bool = true;
                    
                    let point_sides = get_open_sides(&neighbor_dirs, world, &map_data, point, &checked);

                    for y in point.y+1..chunk_top_y+2 {
                        let point_above = Point::new(point.x, y, point.z);
                        
                        match map_data.octree.query_point(point_above) {
                            Some(_) => {

                                let curr_sides = get_open_sides(&neighbor_dirs, world, &map_data, point_above, &checked);

                                if curr_sides.symmetric_difference(&point_sides).count() > 0 {
                                    //if there are more point_sides than curr_sides, ie: if more sides are covered as we go up
                                    if curr_sides.difference(&point_sides).count() == 0 {
                                        draw_top = false;
                                    }
                                    break;
                                } else {

                                    let point_y_in_world = point_above.y as f32 * TILE_DIMENSIONS.y;
                                    let subdivide_for_repeat = is_a_subdivision(point_y_in_world);

                                    if subdivide_for_repeat {
                                        draw_top = false;
                                        // draw_top = true; //comment out when not debugging
                                        break                                                                                                                                           ;
                                    }

                                    if map_coords_to_world(point).y < true_top - 1. && map_coords_to_world(point_above).y + TILE_DIMENSIONS.y > true_top - 1. {
                                        draw_top = false;
                                        break;
                                    }
                                }

                                checked.insert(point_above);
                                top = point_above;
                            },
                            None if y > chunk_top_y => {

                                let chunk_point_above = map_data.get_chunk_point()+Point::y();

                                let chunk_point_above_query = <Read<MapChunkData>>::query()
                                    .filter(tag_value(&chunk_point_above));

                                if let Some(map_data) = chunk_point_above_query.iter(world).next() {
                                    if let Some(_) = map_data.octree.query_point(point_above) {

                                        let curr_sides = get_open_sides(&neighbor_dirs, world, &map_data, point_above, &checked);

                                        if curr_sides.symmetric_difference(&point_sides).count() > 0 {
                                            //if there are more point_sides than curr_sides, ie: if more sides are covered as we go up
                                            if curr_sides.difference(&point_sides).count() == 0 {
                                                draw_top = false;
                                            }

                                        } else {
                                            draw_top = false;
                                        }
                                    }
                                }

                            }
                            None => {
                                break;
                            }
                        }
                    }

                    let mut bottom = point;
                    let chunk_bottom_y = map_data.octree.get_aabb().get_min().y;
                    
                    // let mut draw_bottom: bool = true;

                    for y in (chunk_bottom_y-1..point.y).rev() {

                        let point_below = Point::new(point.x, y, point.z);      

                        match map_data.octree.query_point(point_below) {
                            Some(_) => {

                                let curr_sides = get_open_sides(&neighbor_dirs, world, &map_data, point_below, &checked);
                                
                                if curr_sides.symmetric_difference(&point_sides).count() > 0 {

                                    //if there are more points in point_sides than the current_sides. ie: if sides are getting covered as we go down
                                    if point_sides.difference(&curr_sides).count() > 0 {
                                        bottom = point_below;
                                    }
                                    break;
                                } else {

                                    let point_y_in_world = bottom.y as f32 * TILE_DIMENSIONS.y;
                                    let subdivide_for_repeat = is_a_subdivision(point_y_in_world);

                                    if subdivide_for_repeat {
                                        break;
                                    }

                                    if map_coords_to_world(point).y >= true_top - 1. && true_top - 1. > map_coords_to_world(point_below).y {
                                        break;
                                    } 
                                }

                                checked.insert(point_below);
                                bottom = point_below;
                            },
                            None if y < chunk_bottom_y => {

                                let chunk_point_below = map_data.get_chunk_point() - Point::y();

                                let chunk_point_below_query = <Read<MapChunkData>>::query().filter(tag_value(&chunk_point_below));

                                if let Some(map_data) = chunk_point_below_query.iter(world).next() {

                                    if let Some(_) = map_data.octree.query_point(point_below) {
                                        let curr_sides = get_open_sides(&neighbor_dirs, world, &map_data, point_below, &checked);
                                                                                
                                        if curr_sides.symmetric_difference(&point_sides).count() > 0 {

                                            //if there are more points in point_sides than the current_sides. ie: if sides are getting covered as we go down
                                            if point_sides.difference(&curr_sides).count() > 0 {
                                                bottom = point_below;
                                            }
                                        }
                                    }
                                }
                            },
                            None => break
                            
                        }

                    }

                    // godot_print!("Point {:?}'s top is {:?}", point.y, top.y);
                    // godot_print!("Point {:?}'s bottom is {:?}", point.y, bottom.y);

                    // draw_top = true;

                    let open_sides = get_open_sides(&neighbor_dirs, world, &map_data, top, &checked);

                    let world_point = map_coords_to_world(top);

                    let top_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                    let top_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                    let bottom_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);
                    let bottom_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);

                    let mut center = bottom_left + (top_right - bottom_left) / 2.;
                    
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

                            mesh_data.uv2s.push(&Vector2::default());
                            mesh_data.uv2s.push(&Vector2::default());
                            mesh_data.uv2s.push(&Vector2::default());
                            mesh_data.uv2s.push(&Vector2::default());

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

                        let corners = [
                            top_right, 
                            top_left, 
                            bottom_left, 
                            bottom_right
                        ];

                        let mut border_points: Vec<Vector3> = Vec::with_capacity(12);
                        let mut face_points: Vec<Vector3> = Vec::with_capacity(12);

                        let corners_len = corners.len();
                        let mut i = 0;
                        while i < corners_len {

                            let right = corners[i];
                            let left = corners[(i + 1) % corners_len];

                            let dir = get_direction_of_edge(right, left, center);
                            let bevel = Vector3::new(dir.x as f32, dir.y as f32, dir.z as f32) * BEVEL_SIZE / 2.;

                            let right_dir = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_2) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                            let right_dir = Point::new(right_dir.x as i32, right_dir.y as i32, right_dir.z as i32);
                            
                            let left_dir = -right_dir;

                            let mut scale_origin = center;
                            let mut scale_size = 1.-BEVEL_SIZE * 2.;
                            let corner_scale = scale_size * 1.2;

                            // Define top face points based on which sides are exposed or not.
                            if open_sides.contains(&dir) {

                                // godot_print!("prev_dir = {:?} dir = {:?} next_dir = {:?}", right_dir, dir, left_dir);
                                
                                let mut adj: Vector3 = bevel;
                                let mut corner: Option<Vector3> = None;

                                if !open_sides.contains(&right_dir) && !open_sides.contains(&left_dir) {
                                    scale_size = 1.;
                                    scale_origin = (left + right) / 2.;
                                    adj = -bevel;

                                } else if !open_sides.contains(&right_dir) {
                                    scale_origin = right;
                                    scale_size = 1.-BEVEL_SIZE;
                                    corner = Some(scale_from_origin(left, center, corner_scale));
                                    adj = -bevel;

                                } else if !open_sides.contains(&left_dir) {
                                    scale_origin = left;
                                    scale_size = 1.-BEVEL_SIZE;
                                    adj = -bevel;

                                } else {
                                    corner = Some(scale_from_origin(left, scale_origin, corner_scale));
                                }

                                let mut scaled_right = scale_from_origin(right, scale_origin, scale_size);
                                let mut scaled_left = scale_from_origin(left, scale_origin, scale_size);

                                scaled_right += adj;
                                scaled_left += adj;

                                face_points.append(&mut vec![scaled_right, scaled_left]);
                                if let Some(corner) = corner {
                                    face_points.push(corner);
                                }

                            } else {

                                let right_diag = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_4) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                                let right_diag = Point::new(right_diag.x.round() as i32, right_diag.y.round() as i32, right_diag.z.round() as i32);

                                let left_diag = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), -std::f32::consts::FRAC_PI_4) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                                let left_diag = Point::new(left_diag.x.round() as i32, left_diag.y.round() as i32, left_diag.z.round() as i32);

                                let mut adj: Option<Vector3> = None;

                                if !open_sides.contains(&left_dir) && !open_sides.contains(&right_dir) {
                                    if !open_sides.contains(&right_diag) && !open_sides.contains(&left_diag) {
                                        scale_size = 1.;

                                    } else if !open_sides.contains(&left_diag) {
                                        scale_origin = left;
                                        scale_size = 1.-BEVEL_SIZE / 2.;                                    
                                    } else {
                                        scale_origin = (right + left) / 2.;
                                        scale_size = 1.-BEVEL_SIZE;
                                    }
                                } else if !open_sides.contains(&left_dir) {
                                    if !open_sides.contains(&left_diag){
                                        scale_origin = left;
                                        scale_size = 1.-BEVEL_SIZE / 2.;
                                    } else {
                                        scale_origin = left;
                                        adj = Some(Vector3::new(right_dir.x as f32, right_dir.y as f32, right_dir.z as f32) * BEVEL_SIZE / 2.);
                                        scale_size = 1.-BEVEL_SIZE;
                                    }
                                } else {
                                    scale_origin = (right + left) / 2.;
                                    scale_size = 1.-BEVEL_SIZE;
                                }

                                let mut scaled_right = scale_from_origin(right, scale_origin, scale_size);
                                let mut scaled_left = scale_from_origin(left, scale_origin, scale_size);

                                if let Some(adj) = adj {
                                    scaled_right += adj;
                                    scaled_left += adj;
                                }

                                face_points.append(&mut vec![scaled_right, scaled_left]);

                            }

                            i += 1;
                        }

                        let mut face_points_final: Vec<Vector3> = Vec::with_capacity(12);
                        //keep track of the indices of the face points so that we can use them again
                        // in the bezel curve for the top face
                        let mut face_point_indices: Vec<i32> = Vec::with_capacity(12);

                        let face_points_len = face_points.len();
                        let mut i = 0;
                        while i < face_points_len {

                            let right = face_points[i];
                            let left = face_points[(i + 1) % face_points_len];

                            if (right - left).length() > std::f32::EPSILON {

                                // godot_print!("right {:?}", right);

                                face_points_final.push(right);
                            }

                            i += 1;
                        }

                        mesh_data.verts.push(&center);
                        mesh_data.uvs.push(&Vector2::new(TILE_SIZE / 2., TILE_SIZE / 2.));
                        mesh_data.uv2s.push(&Vector2::default());
                        mesh_data.normals.push(&Vector3::new(0.,1.,0.));
                        offset += 1;

                        let face_points_final_len = face_points_final.len();
                        let mut i = 0;
                        let begin = offset;
                        while i < face_points_final_len {

                            let right = face_points_final[i % face_points_final_len];

                            let u = (right.x - world_point.x).abs() * TILE_SIZE;
                            let v = (right.z - world_point.z).abs() * TILE_SIZE;

                            if draw_top {
                                mesh_data.verts.push(&right);
                                mesh_data.uvs.push(&Vector2::new(u, v));
                                mesh_data.uv2s.push(&Vector2::default());
                                mesh_data.normals.push(&Vector3::new(0., 1., 0.));

                                face_point_indices.push(begin + i as i32);

                                offset += 1;

                                if i > 0 && i < face_points_final_len - 1 {
                                    mesh_data.indices.push(begin);
                                    mesh_data.indices.push(begin + i as i32);
                                    mesh_data.indices.push(begin + (i as i32 + 1) % face_points_final_len as i32);
                                }
                            }

                            i+= 1;
                        }

                        //defining the curve to the top face
                        let mut i = 0;
                        let begin = offset;
                        while i < face_points_final_len {

                            let right_index = i;
                            let left_index = (i + 1) % face_points_final_len;

                            let right = face_points_final[right_index];
                            let left = face_points_final[left_index];

                            let dir = get_direction_of_edge(right, left, center);

                            let right_rot = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_2);
                            let left_rot = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), -std::f32::consts::FRAC_PI_2);
                            let right_dir = right_rot * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                            let right_dir = Point::new(right_dir.x as i32, right_dir.y as i32, right_dir.z as i32);
                            
                            let left_dir = -right_dir;

                            let right_diag= nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_4) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                            let right_diag = Point::new(right_diag.x.round() as i32, right_diag.y.round() as i32, right_diag.z.round() as i32);    

                            let left_diag= nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), -std::f32::consts::FRAC_PI_4) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                            let left_diag = Point::new(left_diag.x.round() as i32, left_diag.y.round() as i32, left_diag.z.round() as i32);

                            let mut scaled_right = scale_from_origin(right, center, 1./(1.-BEVEL_SIZE));
                            let mut scaled_left = scale_from_origin(left, center, 1./(1.-BEVEL_SIZE));

                            //change the origin of our scale for when certain sides are exposed or not
                            if !open_sides.contains(&dir) && (open_sides.contains(&right_dir) || open_sides.contains(&right_diag)){

                                let middle = if right_dir.x.abs() > right_dir.z.abs() {
                                    Vector3::new(right_dir.x as f32, right_dir.y as f32, right_dir.z as f32) * TILE_DIMENSIONS.x / 2.
                                } else {
                                    Vector3::new(right_dir.x as f32, right_dir.y as f32, right_dir.z as f32) * TILE_DIMENSIONS.z / 2.
                                };

                                let middle = left_rot * Vector3D::new(middle.x, middle.y, middle.z);

                                let middle = Vector3::new(middle.x, middle.y, middle.z) + center;

                                scaled_right = scale_from_origin(right, middle,  1./(1.-BEVEL_SIZE));

                            } else if open_sides.contains(&dir) && !open_sides.contains(&right_dir) {
                                if (left-right).length() > 0.5 {

                                    let middle = if dir.x.abs() > dir.z.abs() {
                                        Vector3::new(dir.x as f32, dir.y as f32, dir.z as f32) * TILE_DIMENSIONS.x / 2.
                                    } else {
                                        Vector3::new(dir.x as f32, dir.y as f32, dir.z as f32) * TILE_DIMENSIONS.z / 2.
                                    };

                                    let middle = right_rot * Vector3D::new(middle.x, middle.y, middle.z);

                                    let middle = Vector3::new(middle.x, middle.y, middle.z) + center;

                                    scaled_right = scale_from_origin(right, middle, 1./(1.-BEVEL_SIZE));

                                }
                            }

                            if open_sides.contains(&dir) && !open_sides.contains(&left_dir) {
                                if (left-right).length() > 0.5 {

                                    let middle = if dir.x.abs() > dir.z.abs() {
                                        Vector3::new(dir.x as f32, dir.y as f32, dir.z as f32) * TILE_DIMENSIONS.x / 2.
                                    } else {
                                        Vector3::new(dir.x as f32, dir.y as f32, dir.z as f32) * TILE_DIMENSIONS.z / 2.
                                    };

                                    let middle = left_rot * Vector3D::new(middle.x, middle.y, middle.z);

                                    let middle = Vector3::new(middle.x, middle.y, middle.z) + center;

                                    scaled_left = scale_from_origin(left, middle,  1./(1.-BEVEL_SIZE));
                                }
                            }

                            //draw the curves
                            if draw_top {

                                scaled_right.y -= BEVEL_SIZE / 2.;
                                scaled_left.y -= BEVEL_SIZE / 2.;

                                let u = (scaled_right.x - world_point.x).abs() * TILE_SIZE;
                                let v = (scaled_right.z - world_point.z).abs() * TILE_SIZE;

                                mesh_data.verts.push(&scaled_right);
                                mesh_data.uvs.push(&Vector2::new(u, v));
                                mesh_data.uv2s.push(&Vector2::default());

                                let mut normal = (scaled_right + scaled_left) / 2.;
                                normal.y = center.y;
                                normal = (normal - center).normalize();

                                mesh_data.normals.push(&(normal).normalize());

                                offset += 1;

                                let face_right_index = face_point_indices[right_index];
                                let face_left_index = face_point_indices[left_index]; 

                                if open_sides.contains(&dir) || (!open_sides.contains(&right_dir) && open_sides.contains(&right_diag)) || (!open_sides.contains(&left_dir) && open_sides.contains(&left_diag)){
                                    mesh_data.indices.push(face_left_index);
                                    mesh_data.indices.push(face_right_index);
                                    mesh_data.indices.push(begin + left_index as i32);

                                    mesh_data.indices.push(face_right_index);
                                    mesh_data.indices.push(begin + right_index as i32);
                                    mesh_data.indices.push(begin + left_index as i32);
                                }

                            }

                            border_points.push(scaled_right);
                            border_points.push(scaled_left);
                            
                            i += 1;
                        }

                        //define the vertices for the walls
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

                                //top
                                mesh_data.verts.push(&border_point);

                                //bottom

                                let bottom_point = *border_point - Vector3::new(0., height, 0.);
                                mesh_data.verts.push(&(bottom_point));
                                
                                //define the uvs for the walls on every other iteration 
                                if i % 2 == 0 {

                                    let diff = *next_point - *border_point;

                                    let mut normal_origin = (*next_point + *border_point) / 2.;
                                    normal_origin.y = center.y;
                                    normal_origin = (normal_origin - center).normalize();

                                    let mut normal_origin_bp = normal_origin;
                                    normal_origin_bp.y = border_point.y;
                                    let mut normal_origin_bot = normal_origin;
                                    normal_origin_bot.y = bottom_point.y;
    
                                    mesh_data.normals.push(&(normal_origin).normalize());
                                    mesh_data.normals.push(&(normal_origin).normalize());

                                    mesh_data.normals.push(&(normal_origin).normalize());
                                    mesh_data.normals.push(&(normal_origin).normalize());

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
                                    
                                    let vert_offset = if bottom < START_REPEAT_BELOW_HEIGHT  {
                                        (bottom / REPEAT_AMOUNT_BELOW).floor() * REPEAT_AMOUNT_BELOW * TILE_SIZE
                                    } else if bottom - START_REPEAT_ABOVE_HEIGHT >= START_REPEAT_BELOW_HEIGHT {
                                        ((((bottom - START_REPEAT_ABOVE_HEIGHT) / REPEAT_AMOUNT_ABOVE).floor() * REPEAT_AMOUNT_ABOVE) - REPEAT_AMOUNT_BELOW) * TILE_SIZE
                                    } else {
                                        - REPEAT_AMOUNT_BELOW * TILE_SIZE
                                    }; //height * TILE_SIZE;

                                    mesh_data.uvs.push(&Vector2::new(u,-1.-height * TILE_SIZE - bottom * TILE_SIZE + vert_offset)); //bottom of face
                                    mesh_data.uvs.push(&Vector2::new(u,-1.-bottom * TILE_SIZE + vert_offset)); //top of face

                                    mesh_data.uvs.push(&Vector2::new(next_u,-1.-height * TILE_SIZE - bottom * TILE_SIZE + vert_offset)); //bottom of face
                                    mesh_data.uvs.push(&Vector2::new(next_u,-1.-bottom * TILE_SIZE + vert_offset)); //top of face

                                    //define the uvs for the grass overhang textures
                                    if map_coords_to_world(point).y + std::f32::EPSILON >= true_top - 1. {

                                        if dir.z.abs() > 0 {

                                            u = TILE_SIZE + (border_point.x - world_point.x).abs() * TILE_SIZE;
    
                                            next_u = TILE_SIZE + (next_point.x - world_point.x).abs() * TILE_SIZE ;
    
                                            if diff.x > 0. {
    
                                                u = -u;
                                                next_u = -next_u;
    
                                            }
    
                                        } else if dir.x.abs() > 0 {
                                            u = TILE_SIZE + (border_point.z - world_point.z).abs() * TILE_SIZE;
    
                                            next_u = TILE_SIZE + (next_point.z - world_point.z).abs() * TILE_SIZE ;
    
                                            if diff.z > 0. {
    
                                                u = -u;
                                                next_u = -next_u;
    
                                            }
                                        }

                                        if u < 0. {
                                            u = (1. - u) % 1.;
                                        }

                                        if next_u < 0. {
                                            next_u = (1. - next_u) % 1.;
                                        }

                                        let top_v = TILE_SIZE * (true_top - top);
                                        let bottom_v = TILE_SIZE * (true_top - bottom);

                                        mesh_data.uv2s.push(&Vector2::new(u, top_v));
                                        mesh_data.uv2s.push(&Vector2::new(u, bottom_v));

                                        mesh_data.uv2s.push(&Vector2::new(next_u, top_v));
                                        mesh_data.uv2s.push(&Vector2::new(next_u, bottom_v));

                                    } else {
                                        mesh_data.uv2s.push(&Vector2::default());
                                        mesh_data.uv2s.push(&Vector2::default());
                                        
                                        mesh_data.uv2s.push(&Vector2::default());
                                        mesh_data.uv2s.push(&Vector2::default());
                                    }

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

            let mut to_changed: Vec<Entity> = Vec::new();
            let mut to_change: Vec<Entity> = Vec::new();

            for (entity, (map_data,change)) in map_query.iter_entities_unchecked(world) {
                to_changed.push(entity);

                //only manually change neighbors if it is a direct change
                if change.0 == ChangeType::Direct {
                    for dir in &all_dirs {
                        
                        let neighbor_chunk_pt = map_data.get_chunk_point() + dir;

                        let neighbor_chunk_query = <Read<MapChunkData>>::query()
                            .filter(tag_value(&neighbor_chunk_pt));

                        for (entity, _) in neighbor_chunk_query.iter_entities_unchecked(world) {
                            
                            to_change.push(entity);
                        }
                    }
                }
            }

            for entity in &*to_change {
                world.add_tag(*entity, ManuallyChange(ChangeType::Indirect)).unwrap();
            }

            for entity in &*to_changed {
                world.remove_tag::<ManuallyChange>(*entity).unwrap();
            }
        }

    })
} 

/// Get the true top of this vertical column of tiles regardless of chunk subdivisions
fn get_true_top(pt: Point, world: &legion::world::World,map_data: &MapChunkData, _checked: &HashSet<Point>) -> Point {
    let mut true_top = pt;

    let chunk_max = map_data.octree.get_aabb().get_max();
    let chunk_min = map_data.octree.get_aabb().get_min();

    if true_top.y < chunk_min.y + 1 {
        true_top.y = chunk_min.y;
    }

    while true_top.y <= chunk_max.y+1 {

        match map_data.octree.query_point(true_top) {
            Some(_) => {

            },
            None if true_top.y > chunk_max.y => {
                let chunk_pt_above = map_data.get_chunk_point()+Point::y();

                match <Read<MapChunkData>>::query().filter(tag_value(&chunk_pt_above)).iter(world).next() {
                    Some(map_data) => {
                        true_top = get_true_top(true_top, world, &map_data, _checked);
                        return true_top;
                    },
                    None => break
                }
            },
            None => break
        }

        true_top.y += 1;

    }

    true_top
}

pub fn scale_from_origin(pt: Vector3, origin: Vector3, scale_amount: f32) -> Vector3 {
    let mut pt = pt;
    pt -= origin;
    pt *= scale_amount;
    pt + origin
} 

pub fn get_open_sides(neighbor_dirs: &[Point; 8], world: &legion::world::World, map_data: &MapChunkData, point: Point, checked: &HashSet<Point>) -> HashSet<Point> {
    // let mut open_sides: HashSet<Point> = HashSet::new();
    let chunk_max = map_data.octree.get_aabb().get_max();
    let chunk_min = map_data.octree.get_aabb().get_min();
    
    let (tx, rx) = mpsc::channel::<Point>();

    // for dir in neighbor_dirs {
    neighbor_dirs.par_iter().for_each_with(tx, |tx, dir| {

        let neighbor = point + *dir;

        if checked.contains(&neighbor) {
            return {}
        }

        match map_data.octree.query_point(neighbor) {
            Some(_) => return {},
            None => {

                match map_data.octree.get_aabb().contains_point(neighbor) {
                    false => {

                        let mut adj_dir = *dir;

                        if neighbor.x > chunk_max.x {
                            adj_dir.x = 1;    
                        } else if neighbor.x < chunk_min.x {
                            adj_dir.x = -1;
                        } else {
                            adj_dir.x = 0;
                        }
                        
                        if neighbor.z > chunk_max.z {
                            adj_dir.z = 1;
                        } else if neighbor.z < chunk_min.z {
                            adj_dir.z = -1; 
                        } else {
                            adj_dir.z = 0;
                        }

                        let chunk_point_dir = map_data.get_chunk_point() + adj_dir;

                        let chunk_point_dir_query = <Read<MapChunkData>>::query()
                            .filter(tag_value(&chunk_point_dir));

                        match chunk_point_dir_query.iter(world).next() {
                            Some(map_data) => {
                                
                                match map_data.octree.query_point(neighbor) {
                                    Some(_) => return {},
                                    None => {
                                        tx.send(*dir).unwrap();
                                    }
                                }

                            },
                            None => { tx.send(*dir).unwrap(); }
                        }
                    },
                    true => { tx.send(*dir).unwrap(); }
                }
            }
        }
    });

    let open_sides: HashSet<Point> = rx.into_iter().collect();

    open_sides
}

fn is_a_subdivision(point_y: f32) -> bool {
    (point_y >= START_REPEAT_ABOVE_HEIGHT && (point_y % REPEAT_AMOUNT_ABOVE - START_REPEAT_ABOVE_HEIGHT) % REPEAT_AMOUNT_ABOVE == 0.) 
        || (point_y <= START_REPEAT_BELOW_HEIGHT && point_y % REPEAT_AMOUNT_BELOW == 0.)
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