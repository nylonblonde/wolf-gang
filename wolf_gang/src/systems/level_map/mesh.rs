/// Handles the creation and defining of mesh nodes in Godot

use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use crate::systems::custom_mesh; 
use crate::geometry::aabb;
use crate::collections::octree::PointData;

use gdnative::prelude::*;

use nalgebra;

use legion::*;

type AABB = aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;
type Vector3D = nalgebra::Vector3<f32>;

use rayon::prelude::*;
use std::sync::mpsc;

use super::*;

const TILE_PIXELS: f32 = 64.;
const SHEET_PIXELS: f32 = 1024.;
const TILE_SIZE: f32 = TILE_PIXELS/SHEET_PIXELS;
const BEVEL_SIZE: f32 = 0.2;
const START_REPEAT_ABOVE_HEIGHT: f32 = 2.;
const START_REPEAT_BELOW_HEIGHT: f32 = 0.;
const REPEAT_AMOUNT_ABOVE: f32 = 2.;
const REPEAT_AMOUNT_BELOW: f32 = 1.;
const WALL_VERTICAL_OFFSET: f32 = 9.;

lazy_static!{
    pub static ref NEIGHBOR_DIRS: [Point; 8] = [
            Point::x(),
            -Point::x(),
            Point::z(),
            -Point::z(),
            Point::x()+Point::z(),
            -Point::x()+Point::z(),
            -Point::x()-Point::z(),
            Point::x()-Point::z()
        ];
}

/// Adds additional required components
pub fn create_add_components_system() -> impl systems::Runnable {
    
    SystemBuilder::new("map_mesh_add_components_system")
        .with_query(<Entity>::query()
            .filter(component::<custom_mesh::MeshData>())
            .filter(component::<MapChunkData>() & !component::<custom_mesh::Material>())
        )
        .build(|commands, world, _, query| {

            let entities = query.iter(world).map(|entity| *entity).collect::<Vec<Entity>>();

            for entity in entities {

                commands.exec_mut(move |world| {
                    if let Some(mut entry) = world.entry(entity) {
                        entry.add_component(custom_mesh::Material::from_str("res://materials/ground.material"));
                    }
                })
            }
        })
}

pub fn create_drawing_system() -> Box<dyn FnMut(&mut World, &mut Resources)> {

    let mut changed_query = <(Entity, Read<MapChunkData>, Read<ManuallyChange>)>::query();
    let mut map_query = <(Entity, Read<MapChunkData>, Read<Point>)>::query();
    let mut write_mesh_query = <(Entity, Write<MapMeshData>, Write<custom_mesh::MeshData>, Read<ManuallyChange>)>::query();

    Box::new(move |world, resources| {

    // SystemBuilder::new("map_mesh_drawing_system")
    //     .with_query(<(Entity, Read<MapChunkData>, Read<ManuallyChange>)>::query())
    //     .with_query(<(Entity, Read<MapChunkData>, Read<Point>)>::query())
    //     .with_query(<(Entity, Write<MapMeshData>, Write<custom_mesh::MeshData>, Read<ManuallyChange>)>::query())
    //     .build(|commands, world, _, queries| { 

            // let (changed_query, map_query, write_mesh_query) = queries;

            let map_datas = map_query.iter(world)
                .map(|(entity, map_data, point)| (*entity, (*map_data).clone(), *point))
                .collect::<Vec<(Entity, MapChunkData, Point)>>();

            let entities = changed_query.iter(world)
                .map(|(entity, map_data, change)| (*entity, (*map_data).clone(), (*change).clone()))
                .collect::<Vec<(Entity, MapChunkData, ManuallyChange)>>();

            let (map_mesh_tx, map_mesh_rx) = mpsc::channel::<(Entity, HashMap<usize, VertexData>)>();

            entities.par_iter().for_each_with(map_mesh_tx, |map_mesh_tx, (entity, map_data, change)| {

                let now = std::time::Instant::now();

                for i in 0..change.ranges.len() {

                    let change = &change.ranges[i];
                    
                    let change_aabb = match change {
                        ChangeType::Direct(aabb) | ChangeType::Indirect(aabb) => {
                            println!("Change is {:?}", change);
                            get_aabb_change_in_range(*aabb, map_data.octree.get_aabb())
                        },
                    };

                    let aabb = map_data.octree.get_aabb();
                    let max = aabb.get_max();
                    let min = aabb.get_min();

                    let change_min = change_aabb.get_min();
                    let area = change_aabb.dimensions.x * change_aabb.dimensions.z;
                    
                    let checked: Arc<Mutex<HashSet<Point>>> = Arc::new(Mutex::new(HashSet::new()));
                    let (vert_data_tx, vert_data_rx) = mpsc::channel::<(usize, VertexData)>();

                    (0..0+area).collect::<Vec<i32>>().par_iter().for_each_with(vert_data_tx, |vert_data_tx, i| {

                        let x = (i % change_aabb.dimensions.x) + change_min.x;
                        let z = (i / change_aabb.dimensions.x) + change_min.z;

                        let (checked_tx, checked_rx) = mpsc::channel::<Point>();
                        let (vertex_tx, vertex_rx) = mpsc::channel::<VertexData>();

                        map_data.octree.query_range(AABB::from_extents(Point::new(x, min.y, z), Point::new(x, max.y, z)))
                            .par_iter()
                            .for_each_with(
                                (checked.clone(), checked_tx, vertex_tx), 
                                |(checked, checked_tx, vertex_tx), tile| {

                                    let point = tile.get_point();

                                    let checked = {
                                        let mut checked_lock = checked.lock().unwrap();
                                        let checked = &mut *checked_lock;
                                        
                                        checked_tx.send(point).unwrap();

                                        checked.clone()
                                    };

                                    let mut true_top: Option<Vector3D> = None;

                                    let mut draw_top: bool = true;

                                    let point_sides = get_open_sides(&map_datas, &map_data, point, &checked);

                                    let point_above = point + Point::y();

                                    //If this tile does not match any of the conditions that would make it a top facing tile
                                    if let false = match map_data.octree.query_point(point_above) {
                                        Some(_) => {
                                            let curr_sides = get_open_sides(&map_datas, &map_data, point_above, &checked);

                                            if curr_sides.symmetric_difference(&point_sides).count() > 0 {
                                                //if there are more point_sides than curr_sides, ie: if more sides are covered as we go up
                                                if curr_sides.difference(&point_sides).count() == 0 {
                                                    draw_top = false;
                                                }
                                                true
                                            } else {

                                                let point_y_in_world = point_above.y as f32 * TILE_DIMENSIONS.y;
                                                let subdivide_for_repeat = is_a_subdivision(point_y_in_world);

                                                if subdivide_for_repeat {
                                                    draw_top = false;
                                                    // draw_top = true; //comment out when not debugging
                                                    true                                                                                                                                          
                                                } else {
                                                    let tt = super::map_coords_to_world(get_true_top(point, &map_datas, &map_data, &checked));
                                                    true_top = Some(tt);

                                                    let diff = tt.y - 1. - map_coords_to_world(point_above).y;

                                                    //if approx zero
                                                    if diff > -std::f32::EPSILON && diff < std::f32::EPSILON {
                                                        draw_top = false;
                                                        true
                                                    } else {
                                                        false
                                                    }
                                                }
                                            }
                                        },
                                        None if point_above.y > max.y => {

                                            let chunk_point_above = map_data.get_chunk_point()+Point::y();

                                            if let Some((_, map_data, _)) = map_datas.iter().filter(|(_,_,pt)| *pt == chunk_point_above).next() {
                                                if let Some(_) = map_data.octree.query_point(point_above) {

                                                    let curr_sides = get_open_sides(&map_datas, &map_data, point_above, &checked);

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
                                            true
                                        }
                                        None => true
                                    } {
                                        return{}
                                    }

                                    let mut offset = 0;

                                    // We do this as a SLIGHT optimization, there's no sense in calculating this for EVERY tile if it's not going to be worked on
                                    // but it's possible it was already calculated when determining the top. If it wasn't, we have to do it now
                                    if let None = true_top {
                                        true_top = Some(super::map_coords_to_world(get_true_top(point, &map_datas, &map_data, &checked)));
                                    }

                                    let mut bottom = point;                    
                                    let mut _draw_bottom: bool = true;

                                    //Get the bottom of this piece
                                    for y in (min.y-1..point.y).rev() {

                                        let point_below = Point::new(point.x, y, point.z);      

                                        match map_data.octree.query_point(point_below) {
                                            Some(_) => {

                                                let curr_sides = get_open_sides(&map_datas, &map_data, point_below, &checked);
                                                
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
                                                    let tt = true_top.unwrap();
                                                    if map_coords_to_world(point).y >= tt.y - 1. && tt.y - 1. > map_coords_to_world(point_below).y {
                                                        break;
                                                    } 
                                                }

                                                checked_tx.send(point_below).unwrap();
                                                bottom = point_below;
                                            },
                                            None if y < min.y => {

                                                let chunk_point_below = map_data.get_chunk_point() - Point::y();

                                                if let Some((_, map_data, _)) = map_datas.iter().filter(|(_,_,pt)| *pt == chunk_point_below).next() {

                                                    if let Some(_) = map_data.octree.query_point(point_below) {
                                                        let curr_sides = get_open_sides(&map_datas, &map_data, point_below, &checked);
                                                                                                
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

                                    // draw_top = true;

                                    let world_point = map_coords_to_world(point);

                                    let top_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                                    let top_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z+TILE_DIMENSIONS.z);
                                    let bottom_left = Vector3::new(world_point.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);
                                    let bottom_right = Vector3::new(world_point.x+TILE_DIMENSIONS.x, world_point.y+TILE_DIMENSIONS.y, world_point.z);

                                    let mut center = bottom_left + (top_right - bottom_left) / 2.;
                                    
                                    let mut vertex_data = VertexData::default();

                                    // if there are no open sides, all we have to draw is a simple 2 triangle face
                                    if point_sides.is_empty() {

                                        if draw_top { 

                                            vertex_data.verts.extend(&[
                                                top_right,
                                                top_left,
                                                bottom_left,
                                                bottom_right
                                            ]);

                                            vertex_data.uvs.extend(&[
                                                Vector2::new(TILE_SIZE, TILE_SIZE),
                                                Vector2::new(0., TILE_SIZE),
                                                Vector2::new(0.,0.),
                                                Vector2::new(TILE_SIZE, 0.)
                                            ]);

                                            vertex_data.uv2s.extend(&[
                                                Vector2::default(),
                                                Vector2::default(),
                                                Vector2::default(),
                                                Vector2::default(),
                                            ]);

                                            vertex_data.normals.extend(&[
                                                Vector3::new(0.,1.,0.),
                                                Vector3::new(0.,1.,0.),
                                                Vector3::new(0.,1.,0.),
                                                Vector3::new(0.,1.,0.),
                                            ]);

                                            vertex_data.indices.extend(&[
                                                2,0,1,
                                                3,0,2
                                            ]);

                                            //Don't need to increase the offset here as this is all that would be drawn
                                            // offset += 4;
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
                                        for i in 0..corners_len {

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
                                            if point_sides.contains(&dir) {

                                                // godot_print!("prev_dir = {:?} dir = {:?} next_dir = {:?}", right_dir, dir, left_dir);
                                                
                                                let mut adj: Vector3 = bevel;
                                                let mut corner: Option<Vector3> = None;

                                                if !point_sides.contains(&right_dir) && !point_sides.contains(&left_dir) {
                                                    scale_size = 1.;
                                                    scale_origin = (left + right) / 2.;
                                                    adj = -bevel;

                                                } else if !point_sides.contains(&right_dir) {
                                                    scale_origin = right;
                                                    scale_size = 1.-BEVEL_SIZE;
                                                    corner = Some(scale_from_origin(left, center, corner_scale));
                                                    adj = -bevel;

                                                } else if !point_sides.contains(&left_dir) {
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

                                                face_points.extend(&[scaled_right, scaled_left]);
                                                if let Some(corner) = corner {
                                                    face_points.push(corner);
                                                }

                                            } else {

                                                let right_diag = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), std::f32::consts::FRAC_PI_4) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                                                let right_diag = Point::new(right_diag.x.round() as i32, right_diag.y.round() as i32, right_diag.z.round() as i32);

                                                let left_diag = nalgebra::Rotation3::<f32>::from_axis_angle(&Vector3D::y_axis(), -std::f32::consts::FRAC_PI_4) * Vector3D::new(dir.x as f32, dir.y as f32, dir.z as f32);
                                                let left_diag = Point::new(left_diag.x.round() as i32, left_diag.y.round() as i32, left_diag.z.round() as i32);

                                                let mut adj: Option<Vector3> = None;

                                                if !point_sides.contains(&left_dir) && !point_sides.contains(&right_dir) {
                                                    if !point_sides.contains(&right_diag) && !point_sides.contains(&left_diag) {
                                                        scale_size = 1.;

                                                    } else if !point_sides.contains(&left_diag) {
                                                        scale_origin = left;
                                                        scale_size = 1.-BEVEL_SIZE / 2.;                                    
                                                    } else {
                                                        scale_origin = (right + left) / 2.;
                                                        scale_size = 1.-BEVEL_SIZE;
                                                    }
                                                } else if !point_sides.contains(&left_dir) {
                                                    if !point_sides.contains(&left_diag){
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

                                                face_points.extend(&[scaled_right, scaled_left]);

                                            }
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

                                                face_points_final.push(right);
                                            }

                                            i += 1;
                                        }

                                        vertex_data.verts.push(center);
                                        vertex_data.uvs.push(Vector2::new(TILE_SIZE / 2., TILE_SIZE / 2.));
                                        vertex_data.uv2s.push(Vector2::default());
                                        vertex_data.normals.push(Vector3::new(0.,1.,0.));
                                        offset += 1;

                                        let face_points_final_len = face_points_final.len();
                                        let mut i = 0;
                                        let begin = offset;
                                        while i < face_points_final_len {

                                            let right = face_points_final[i % face_points_final_len];

                                            let u = (right.x - world_point.x).abs() * TILE_SIZE;
                                            let v = (right.z - world_point.z).abs() * TILE_SIZE;

                                            if draw_top {
                                                vertex_data.verts.push(right);
                                                vertex_data.uvs.push(Vector2::new(u, v));
                                                vertex_data.uv2s.push(Vector2::default());
                                                vertex_data.normals.push(Vector3::new(0., 1., 0.));

                                                face_point_indices.push(begin + i as i32);

                                                offset += 1;

                                                if i > 0 && i < face_points_final_len - 1 {
                                                    vertex_data.indices.push(begin);
                                                    vertex_data.indices.push(begin + i as i32);
                                                    vertex_data.indices.push(begin + (i as i32 + 1) % face_points_final_len as i32);
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
                                            if !point_sides.contains(&dir) && (point_sides.contains(&right_dir) || point_sides.contains(&right_diag)){

                                                let middle = if right_dir.x.abs() > right_dir.z.abs() {
                                                    Vector3::new(right_dir.x as f32, right_dir.y as f32, right_dir.z as f32) * TILE_DIMENSIONS.x / 2.
                                                } else {
                                                    Vector3::new(right_dir.x as f32, right_dir.y as f32, right_dir.z as f32) * TILE_DIMENSIONS.z / 2.
                                                };

                                                let middle = left_rot * Vector3D::new(middle.x, middle.y, middle.z);

                                                let middle = Vector3::new(middle.x, middle.y, middle.z) + center;

                                                scaled_right = scale_from_origin(right, middle,  1./(1.-BEVEL_SIZE));

                                            } else if point_sides.contains(&dir) && !point_sides.contains(&right_dir) {
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

                                            if point_sides.contains(&dir) && !point_sides.contains(&left_dir) {
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

                                                vertex_data.verts.push(scaled_right);
                                                vertex_data.uvs.push(Vector2::new(u, v));
                                                vertex_data.uv2s.push(Vector2::default());

                                                let mut normal = (scaled_right + scaled_left) / 2.;
                                                normal.y = center.y;
                                                normal = (normal - center).normalize();

                                                vertex_data.normals.push(normal.normalize());

                                                offset += 1;

                                                let face_right_index = face_point_indices[right_index];
                                                let face_left_index = face_point_indices[left_index]; 

                                                if point_sides.contains(&dir) || (!point_sides.contains(&right_dir) && point_sides.contains(&right_diag)) || (!point_sides.contains(&left_dir) && point_sides.contains(&left_diag)){
                                                    vertex_data.indices.push(face_left_index);
                                                    vertex_data.indices.push(face_right_index);
                                                    vertex_data.indices.push(begin + left_index as i32);

                                                    vertex_data.indices.push(face_right_index);
                                                    vertex_data.indices.push(begin + right_index as i32);
                                                    vertex_data.indices.push(begin + left_index as i32);
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

                                                let border_point = border_points[i];

                                                //get the direction
                                                let next_i = (i+1) % border_points_len;

                                                let next_point = border_points.get(next_i).unwrap();

                                                let dir = get_direction_of_edge(border_point, *next_point, center);

                                                //top
                                                vertex_data.verts.push(border_point);

                                                //bottom

                                                let bottom_point = border_point - Vector3::new(0., height, 0.);
                                                vertex_data.verts.push(bottom_point);
                                                
                                                //define the uvs for the walls on every other iteration 
                                                if i % 2 == 0 {

                                                    let diff = *next_point - border_point;

                                                    let mut normal_origin = (*next_point + border_point) / 2.;
                                                    normal_origin.y = center.y;
                                                    normal_origin = (normal_origin - center).normalize();

                                                    let mut normal_origin_bp = normal_origin;
                                                    normal_origin_bp.y = border_point.y;
                                                    let mut normal_origin_bot = normal_origin;
                                                    normal_origin_bot.y = bottom_point.y;
                    
                                                    vertex_data.normals.push(normal_origin.normalize());
                                                    vertex_data.normals.push(normal_origin.normalize());

                                                    vertex_data.normals.push(normal_origin.normalize());
                                                    vertex_data.normals.push(normal_origin.normalize());

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
                                                    
                                                    let true_top = true_top.unwrap().y;

                                                    let mut vert_offset = if bottom < START_REPEAT_BELOW_HEIGHT  {
                                                        (bottom / REPEAT_AMOUNT_BELOW).floor() * REPEAT_AMOUNT_BELOW * TILE_SIZE
                                                    } else if bottom - START_REPEAT_ABOVE_HEIGHT >= START_REPEAT_BELOW_HEIGHT {
                                                        ((((bottom - START_REPEAT_ABOVE_HEIGHT) / REPEAT_AMOUNT_ABOVE).floor() * REPEAT_AMOUNT_ABOVE) - REPEAT_AMOUNT_BELOW) * TILE_SIZE
                                                    } else {
                                                        - REPEAT_AMOUNT_BELOW * TILE_SIZE
                                                    }; //height * TILE_SIZE;

                                                    vert_offset -= WALL_VERTICAL_OFFSET * TILE_SIZE;

                                                    vertex_data.uvs.push(Vector2::new(u,-1.-height * TILE_SIZE - bottom * TILE_SIZE + vert_offset)); //bottom of face
                                                    vertex_data.uvs.push(Vector2::new(u,-1.-bottom * TILE_SIZE + vert_offset)); //top of face

                                                    vertex_data.uvs.push(Vector2::new(next_u,-1.-height * TILE_SIZE - bottom * TILE_SIZE + vert_offset)); //bottom of face
                                                    vertex_data.uvs.push(Vector2::new(next_u,-1.-bottom * TILE_SIZE + vert_offset)); //top of face

                                                    //define the uvs for the grass overhang textures
                                                    if map_coords_to_world(point).y + std::f32::EPSILON > true_top - 1. {

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

                                                        vertex_data.uv2s.push(Vector2::new(u, top_v));
                                                        vertex_data.uv2s.push(Vector2::new(u, bottom_v));

                                                        vertex_data.uv2s.push(Vector2::new(next_u, top_v));
                                                        vertex_data.uv2s.push(Vector2::new(next_u, bottom_v));

                                                    } else {
                                                        vertex_data.uv2s.push(Vector2::default());
                                                        vertex_data.uv2s.push(Vector2::default());
                                                        
                                                        vertex_data.uv2s.push(Vector2::default());
                                                        vertex_data.uv2s.push(Vector2::default());
                                                    }

                                                } 

                                                //if there are only 2 border points, only draw from the first index to avoid drawing both sides since the index will loop around
                                                if border_points_len > 2 || i < border_points_len-1 {
                                                    //only add indices for points that aren't overlapping
                                                    if (*next_point - border_point).length() > std::f32::EPSILON {

                                                        if point_sides.contains(&dir) {

                                                            let j = offset - begin;

                                                            vertex_data.indices.push(j % indices_len + begin);
                                                            vertex_data.indices.push((j+1) % indices_len + begin);
                                                            vertex_data.indices.push((j+2) % indices_len + begin);

                                                            vertex_data.indices.push((j+2) % indices_len + begin);
                                                            vertex_data.indices.push((j+1) % indices_len + begin);
                                                            vertex_data.indices.push((j+3) % indices_len + begin);

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

                                vertex_tx.send(vertex_data).unwrap();

                            }); //end of iterating through tiles in row

                        let mut vertex_data = VertexData::default();

                        let mut offset: i32 = 0;
                        for received in vertex_rx {

                            let length = received.verts.len();

                            vertex_data.verts.extend(received.verts);
                            vertex_data.normals.extend(received.normals);
                            vertex_data.uvs.extend(received.uvs);
                            vertex_data.uv2s.extend(received.uv2s);
                            vertex_data.indices.extend(received.indices.into_iter().map(|i| i+offset));

                            offset += length as i32;
                        }

                        //convert index to be relative to map_chunk's area
                        let (x, z) = (x - min.x, z - min.z);
                        let i = x + aabb.dimensions.x * z;

                        vert_data_tx.send((i as usize, vertex_data)).unwrap();
                        checked.lock().unwrap().extend(checked_rx.into_iter());
                    }); //end of iterating through rows

                    map_mesh_tx.send((*entity, vert_data_rx.into_iter().collect())).unwrap();

                }

                #[cfg(debug_assertions)]
                println!("Took {:?} milliseconds to complete", now.elapsed().as_millis());

            }); //end of iterating through map chunks

            let map_vert_datas: Arc<Mutex<HashMap<Entity, HashMap<usize, VertexData>>>> = Arc::new(Mutex::new(map_mesh_rx.into_iter().collect()));

            write_mesh_query.for_each_mut(world, |(entity, map_mesh_data, mesh_data, _)| {

                if let Some(map) = map_vert_datas.lock().unwrap().get_mut(&entity) {
                    map.drain().for_each(|(index, data)| {
                        let vertex_data = &mut map_mesh_data.cols[index];
                        vertex_data.replace(data);
                    });
                }

                mesh_data.clear();

                let mut offset = 0;
                for vertex_data in &map_mesh_data.cols {
                    
                    mesh_data.verts.extend(vertex_data.verts.iter());
                    mesh_data.normals.extend(vertex_data.normals.iter());
                    mesh_data.uvs.extend(vertex_data.uvs.iter());
                    mesh_data.uv2s.extend(vertex_data.uv2s.iter());
                    mesh_data.indices.extend(vertex_data.indices.iter().map(|i| i + offset));
                    
                    offset += vertex_data.verts.len() as i32;
                }
                // commands.add_component(*entity, custom_mesh::ManuallyChange{});
            });

            let (to_change_tx, to_change_rx) = mpsc::channel::<(Entity, ChangeType)>();

            if let Some(map) = resources.get::<Map>() {

                entities.par_iter().for_each_with(to_change_tx, |to_change_tx, (_, map_data, change)| {

                    let chunk_pt = map_data.get_chunk_point();

                    change.ranges.iter().for_each(|change| {
                            
                        //only manually change neighbors if it is a direct change
                        if let ChangeType::Direct(aabb) = change {
                            // map.chunks_in_range()

                            let min = aabb.get_min();
                            let max = aabb.get_max();

                            // grab a region below to ensure updates to lower adjacent chunks happen (for the edge lip texture, for instance)
                            let extended_aabb = AABB::from_extents(min - Point::new(1,5,1), max);

                            let neighbors = map.chunks_in_range(map_datas.clone(), extended_aabb);

                            neighbors.into_iter().filter(|(_, neighbor_data)| neighbor_data.get_chunk_point() != chunk_pt).for_each(|(entity, map_data)| {
                                
                                let map_aabb = map_data.octree.get_aabb();

                                //only update if it's adjacent to the changes
                                if aabb.intersects_bounds(map_aabb) {
                                    let aabb = aabb.get_intersection(map_aabb);

                                    to_change_tx.send((entity, ChangeType::Indirect(aabb))).unwrap();
                                }
                            });
                        }
                    });
                });
            }

            //Push indirect changes to their entities
            to_change_rx.into_iter().for_each(|(entity, change)| {
                if let Some(mut entry) = world.entry(entity) {
                    match entry.get_component_mut::<ManuallyChange>() {
                        Ok(manually_change) => { 

                            if let ChangeType::Indirect(change_aabb) = change {

                                let mut push = true;
                                for component_change in &manually_change.ranges {
                                    match component_change {
                                        ChangeType::Direct(range) | ChangeType::Indirect(range) => {

                                            // If the change is the same as another change that has already been processed, forget it
                                            if change_aabb.get_intersection(*range) == change_aabb {
                                                push = false;
                                                break;
                                            }
                                        }
                                    }
                                    
                                }

                                if push {
                                    manually_change.ranges.push(change);
                                }

                            } 
                        },
                        _ => entry.add_component(ManuallyChange{ ranges: vec![change] })
                    }
                }
            });

            entities.into_iter().for_each(|(entity, _, change)| {
                if let Some(mut entry) = world.entry(entity) {
                    //add the custom_mesh ManuallyChange component to tell it to update the mes
                    entry.add_component(custom_mesh::ManuallyChange{});

                    //remove the worked on changes so vertices don't get defined again next frame
                    if let Ok(manually_change) = entry.get_component_mut::<ManuallyChange>() {
                        manually_change.ranges.retain(|range| {

                            let mut ret_val = true; 
                            change.ranges.iter().for_each(|done_range| {
                                if range == done_range {
                                    ret_val = false;
                                }
                            });
                            
                            ret_val
                        });

                        if manually_change.ranges.len() == 0 {
                            entry.remove_component::<ManuallyChange>();
                        }
                    }
                }
            });
            
        })
} 

/// Get the true top of this vertical column of tiles regardless of chunk subdivisions
fn get_true_top(pt: Point, map_datas: &Vec<(Entity, MapChunkData, Point)>, map_data: &MapChunkData, _checked: &HashSet<Point>) -> Point {
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

                match map_datas.iter().filter(|(_,_,pt)| *pt == chunk_pt_above).next() {
                    Some((_, map_data, _)) => {
                        true_top = get_true_top(true_top, map_datas, &map_data, _checked);
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

/// Expands the changed range to include positions on the border of the change, and gets the intersection with aabb to ensure it is within the bounds of the map data's aabb
fn get_aabb_change_in_range(change: AABB, aabb: AABB) -> AABB {
    let expand_aabb = AABB::from_extents(change.get_min() - Point::new(1,1,1), change.get_max() + Point::new(1,1,1));

    let in_range = expand_aabb.get_intersection(aabb);
                        
    in_range
}

pub fn scale_from_origin(pt: Vector3, origin: Vector3, scale_amount: f32) -> Vector3 {
    let mut pt = pt;
    pt -= origin;
    pt *= scale_amount;
    pt + origin
} 

pub fn get_open_sides(map_datas: &Vec<(Entity, MapChunkData, Point)>, map_data: &MapChunkData, point: Point, checked: &HashSet<Point>) -> HashSet<Point> {
    let chunk_max = map_data.octree.get_aabb().get_max();
    let chunk_min = map_data.octree.get_aabb().get_min();
    
    let (tx, rx) = mpsc::channel::<Point>();

    // for dir in neighbor_dirs {
    NEIGHBOR_DIRS.par_iter().for_each_with(tx, |tx, dir| {

        let neighbor = point + *dir;

        if checked.contains(&neighbor) {
            // println!("Checked contains {:?}", neighbor);
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

                        match map_datas.iter().filter(|(_, _, pt)| *pt == chunk_point_dir).next() {
                            Some((_, map_data, _)) => {
                                
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

pub struct MapMeshData {
    cols: Vec<VertexData>
}

impl MapMeshData {
    pub fn new(cols: Vec<VertexData>) -> Self{
        Self {
            cols
        }
    }
}

#[derive(Debug)]
pub struct VertexData {
    verts: Vec<Vector3>,
    normals: Vec<Vector3>,
    uvs: Vec<Vector2>,
    uv2s: Vec<Vector2>,
    indices: Vec<i32>,
}

impl Default for VertexData {
    fn default() -> VertexData {
        VertexData {
            verts: Default::default(),
            normals: Default::default(),
            uvs: Default::default(),
            uv2s: Default::default(),
            indices: Default::default()
        }
    }
}

impl VertexData {
    fn replace(&mut self, other: VertexData) {
        self.verts.clear();
        self.normals.clear();
        self.uvs.clear();
        self.uv2s.clear();
        self.indices.clear();

        self.verts.extend(other.verts.into_iter());
        self.normals.extend(other.normals.into_iter());
        self.uvs.extend(other.uvs.into_iter());
        self.uv2s.extend(other.uv2s.into_iter());
        self.indices.extend(other.indices.into_iter());

    }
}