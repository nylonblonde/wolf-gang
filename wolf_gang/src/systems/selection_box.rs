use gdnative::{
    godot_print, 
    GodotString,
    ImmediateGeometry, 
    Input, 
    Int32Array, 
    MeshInstance, 
    Vector2, 
    Vector2Array, 
    Vector3, 
    Vector3Array
};
use legion::prelude::*;
use nalgebra::{Rotation2, Rotation3};
use num::Float;

use std::cmp::Ordering;

use crate::geometry::aabb;
use crate::camera;
use crate::custom_mesh;
use crate::node;
use crate::level_map::TILE_DIMENSIONS;
use crate::transform;
use crate::input;
use crate::level_map;

type AABB = aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;

type Vector3D = nalgebra::Vector3<f32>;
type Vector2D = nalgebra::Vector2<f32>;

pub struct SelectionBox {
    pub aabb: AABB
}

#[derive(Copy, Clone, PartialEq)]
pub struct CameraAdjustedDirection {
    pub forward: Vector3D,
    pub right: Vector3D
}

impl Default for CameraAdjustedDirection {
    fn default() -> Self {
        CameraAdjustedDirection {
            forward: Vector3D::z(),
            right: Vector3D::x()
        }
    }
}

impl SelectionBox {
    ///Creates a SelectionBox with an aabb at center (0,0,0) with dimensions of (1,1,1).
    pub fn new() -> Self {
        SelectionBox {
            aabb: AABB::new(Point::new(0,0,0), Point::new(1,1,2))
        }
    }

    pub fn from_aabb(aabb: AABB) -> Self {
        SelectionBox {
            aabb
        }
    }
}

#[derive(Default)]
pub struct RelativeCamera(pub String);

pub fn initialize_selection_box(world: &mut World, camera_name: String) {
    let mut node_name = None;
    
    let mut mesh: ImmediateGeometry = ImmediateGeometry::new();

    unsafe { 
        node_name = node::add_node(&mut mesh);
    }

    let node_name = node_name.unwrap();

    world.insert((node_name.clone(),), 
        vec![
            (
                SelectionBox::new(), 
                RelativeCamera(camera_name),
                custom_mesh::MeshData::new(),
                level_map::CoordPos::default(),
                transform::position::Position::default(), 
                CameraAdjustedDirection::default(),
                custom_mesh::Material::from_str("res://select_box.material")
            )
        ]
    );
}

/// Gets the axis closest to forward from a or b, adjusted by adjust_angle around the up axis. We adjust it so that we can smooth out the comparison at 45
/// degree angles.
fn get_forward_closest_axis(a: &Vector3D, b: &Vector3D, forward: &Vector3D, right: &Vector3D, up: &nalgebra::Unit<Vector3D>, adjust_angle: f32) -> std::cmp::Ordering {
    
    let a_dot = a.dot(right);
    let b_dot = b.dot(right);

    let dot = match PartialOrd::partial_cmp(&a_dot, &b_dot) {
        None => 0., //If NaN just set it to 0
        Some(Ordering::Less) => a_dot,
        Some(_) => b_dot
    };

    let dir = match PartialOrd::partial_cmp(&dot, &0.) {
        None => 0., //If NaN just set it to 0
        Some(Ordering::Less) => -1.,
        Some(_) => 1.
    };
 
    let forward = nalgebra::UnitQuaternion::<f32>::from_axis_angle(up, adjust_angle*dir) * forward;

    a.dot(&forward).partial_cmp(
        &b.dot(&forward)
    ).unwrap()
}

pub fn create_orthogonal_dir_thread_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {
    Box::new(|world: &mut World, resources: &mut Resources|{

        let selection_box_query = <(Write<CameraAdjustedDirection>, Read<RelativeCamera>)>::query();

        unsafe {

            for (mut camera_adjusted_dir, relative_cam) in selection_box_query.iter_unchecked(world) {

                let node_name = node::NodeName(relative_cam.0.clone());

                let cam_query = <Read<transform::rotation::Direction>>::query()
                    .filter(tag_value(&node_name) & changed::<transform::rotation::Direction>());

                match cam_query.iter_unchecked(world).next() {
                    Some(dir) => {

                        // Get whichever cartesian direction in the grid is going to act as "forward" based on its closeness to the camera's forward
                        // view.
                        let mut forward = dir.forward;
                        let mut right = dir.right;

                        forward.y = 0.;
                        
                        let adjustment_angle = std::f32::consts::FRAC_PI_8;

                        forward = std::cmp::min_by(Vector3D::z(), 
                            std::cmp::min_by(-Vector3D::z(), 
                                std::cmp::min_by(Vector3D::x(), -Vector3D::x(),
                                    |lh: &Vector3D, rh: &Vector3D| {
                                        get_forward_closest_axis(lh, rh, &forward, &right, &Vector3D::y_axis(), adjustment_angle)
                                    }
                                ), 
                                |lh: &Vector3D, rh: &Vector3D| {
                                    get_forward_closest_axis(lh, rh, &forward, &right, &Vector3D::y_axis(), adjustment_angle)
                                }
                            ), 
                            |lh: &Vector3D, rh: &Vector3D| {
                                get_forward_closest_axis(lh, rh, &forward, &right, &Vector3D::y_axis(), adjustment_angle)
                            }
                        );

                        //calculate right from up and forward
                        right =  nalgebra::UnitQuaternion::<f32>::from_axis_angle(&Vector3D::y_axis(), -std::f32::consts::FRAC_PI_2) * forward;

                        forward = forward.normalize();
                        right = right.normalize();

                        camera_adjusted_dir.forward = forward;
                        camera_adjusted_dir.right = right;
                    },
                    None => {}
                }
            }
        }
    })
} 

/// This function reads input, then moves the center of the selection_box
pub fn create_thread_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {
    Box::new(|world: &mut World, resources: &mut Resources|{
        let time = resources.get::<crate::Time>().unwrap();

        let move_forward = input::Action("move_forward".to_string());
        let move_back = input::Action("move_back".to_string());
        let move_left = input::Action("move_left".to_string());
        let move_right = input::Action("move_right".to_string());
        let move_up = input::Action("move_up".to_string());
        let move_down = input::Action("move_down".to_string());

        let input_query = <(Read<input::InputActionComponent>, Tagged<input::Action>)>::query()
            // .filter(changed::<input::InputActionComponent>())
            .filter(
                tag_value(&move_forward)
                | tag_value(&move_back)
                | tag_value(&move_left)
                | tag_value(&move_right)
                | tag_value(&move_up)
                | tag_value(&move_down)
            );

        //this should be fine as no systems runs in parallel with local thread
        unsafe { 

            for(input_component, action) in input_query.iter_unchecked(world) {                    
                
                if input_component.repeated(time.delta, 0.25) {
                    
                    let selection_box_query = <(Read<RelativeCamera>, Read<CameraAdjustedDirection>, Write<crate::level_map::CoordPos>, Write<transform::position::Position>)>::query()
                        .filter(component::<SelectionBox>());

                    for (relative_cam, camera_adjusted_dir, mut coord_pos, mut position) in selection_box_query.iter_unchecked(world) {

                        let mut movement = Point::zeros();

                        if action.0 == move_forward.0 {
                            movement.z += 1;
                        } else if action.0 == move_back.0 {
                            movement.z -= 1;
                        } else if action.0 == move_left.0 {
                            movement.x -= 1;
                        } else if action.0 == move_right.0 {
                            movement.x += 1;
                        } else if action.0 == move_up.0 {
                            movement.y += 1;
                        } else if action.0 == move_down.0 {
                            movement.y -= 1;
                        }
                        
                        let node_name = node::NodeName(relative_cam.0.clone());

                        let mut adjusted = movement;

                        let forward = camera_adjusted_dir.forward;
                        let right = camera_adjusted_dir.right;

                        adjusted = Point::new(
                            forward.x.round() as i32,
                            0,
                            forward.z.round() as i32
                        ) * movement.z + Point::new(
                            right.x.round() as i32,
                            0,
                            right.z.round() as i32
                        ) * movement.x;
                        // let cam_query = <Read<transform::rotation::Direction>>::query()
                        //     .filter(tag_value(&node_name) & changed::<transform::rotation::Direction>());

                        // //this is close to working, gotta check what I did with old Wolf Gang 
                        // match cam_query.iter_unchecked(world).next() {
                        //     Some(dir) => {

                        //         // Get whichever cartesian direction in the grid is going to act as "forward" based on its closeness to the camera's forward
                        //         // view.
                        //         let mut forward = dir.forward;
                        //         let mut right = dir.right;

                        //         forward.y = 0.;
                                
                        //         let adjustment_angle = std::f32::consts::FRAC_PI_8;

                        //         forward = std::cmp::min_by(Vector3D::z(), 
                        //             std::cmp::min_by(-Vector3D::z(), 
                        //                 std::cmp::min_by(Vector3D::x(), -Vector3D::x(),
                        //                     |lh: &Vector3D, rh: &Vector3D| {
                        //                         get_forward_closest_axis(lh, rh, &forward, &right, &Vector3D::y_axis(), adjustment_angle)
                        //                     }
                        //                 ), 
                        //                 |lh: &Vector3D, rh: &Vector3D| {
                        //                     get_forward_closest_axis(lh, rh, &forward, &right, &Vector3D::y_axis(), adjustment_angle)
                        //                 }
                        //             ), 
                        //             |lh: &Vector3D, rh: &Vector3D| {
                        //                 get_forward_closest_axis(lh, rh, &forward, &right, &Vector3D::y_axis(), adjustment_angle)
                        //             }
                        //         );

                        //         //calculate right from up and forward
                        //         right =  nalgebra::UnitQuaternion::<f32>::from_axis_angle(&Vector3D::y_axis(), -std::f32::consts::FRAC_PI_2) * forward;

                        //         forward = forward.normalize();
                        //         right = right.normalize();

                        //         adjusted = Point::new(
                        //             forward.x.round() as i32,
                        //             0,
                        //             forward.z.round() as i32
                        //         ) * movement.z + Point::new(
                        //             right.x.round() as i32,
                        //             0,
                        //             right.z.round() as i32
                        //         ) * movement.x;

                        //         adjusted.y = movement.y;
                        //     },
                        //     None => {}
                        // };

                        coord_pos.value += adjusted;

                        let coord_pos = crate::level_map::map_coords_to_world(coord_pos.value);
                        position.value = Vector3::new(coord_pos.x, coord_pos.y, coord_pos.z); 
                    }
                }            
            }
        }

        let expand_selection_forward = input::Action("expand_selection_forward".to_string());
        let expand_selection_back = input::Action("expand_selection_back".to_string());
        let expand_selection_left = input::Action("expand_selection_left".to_string());
        let expand_selection_right = input::Action("expand_selection_right".to_string());
        let expand_selection_up = input::Action("expand_selection_up".to_string());
        let expand_selection_down = input::Action("expand_selection_down".to_string());

        let input_query = <(Read<input::InputActionComponent>, Tagged<input::Action>)>::query()
            .filter(
                tag_value(&expand_selection_forward)
                | tag_value(&expand_selection_back)
                | tag_value(&expand_selection_left)
                | tag_value(&expand_selection_right)
                | tag_value(&expand_selection_up)
                | tag_value(&expand_selection_down)
            );

        unsafe { 

            for(input_component, action) in input_query.iter_unchecked(world) {                    
                
                if input_component.repeated(time.delta, 0.25) {
                    
                    let selection_box_query = <(Write<SelectionBox>,)>::query();

                    for (mut selection_box,) in selection_box_query.iter_unchecked(world) {

                        if action == &expand_selection_forward {
                            godot_print!("boop");
                            selection_box.aabb.dimensions.z += 1;
                        } 

                    }
                }
            }
        }
        
    })
}

pub fn create_system() -> Box<dyn Schedulable> {
    
    SystemBuilder::<()>::new("selection_box_system")
        .with_query(<(Read<SelectionBox>, Write<custom_mesh::MeshData>,)>::query()
            .filter(changed::<SelectionBox>(),)
        )
        .build(move |commands, world, resource, queries| {

            for (entity, (selection_box, mut mesh_data)) in queries.iter_entities_mut(&mut *world) {

                mesh_data.verts = Vector3Array::new();
                mesh_data.normals = Vector3Array::new();
                mesh_data.uvs = Vector2Array::new();
                mesh_data.indices = Int32Array::new();

                //offset that the next face will begin on, increments by the number of verts for each face
                //at the end of each loop
                let mut offset = 0;

                let min = selection_box.aabb.get_min();
                let max = selection_box.aabb.get_max();

                //subtract the center to get the "local" min and max
                let min = Vector3D::new(min.x as f32 * TILE_DIMENSIONS.x - selection_box.aabb.center.x as f32 * TILE_DIMENSIONS.x, 
                    min.y as f32 * TILE_DIMENSIONS.y - selection_box.aabb.center.y as f32 * TILE_DIMENSIONS.y, 
                    min.z as f32 * TILE_DIMENSIONS.z - selection_box.aabb.center.z as f32 * TILE_DIMENSIONS.z
                );
                let max = Vector3D::new(max.x as f32 * TILE_DIMENSIONS.x - selection_box.aabb.center.x as f32 * TILE_DIMENSIONS.x, 
                    max.y as f32 * TILE_DIMENSIONS.y - selection_box.aabb.center.y as f32 * TILE_DIMENSIONS.y, 
                    max.z as f32 * TILE_DIMENSIONS.z - selection_box.aabb.center.z as f32 * TILE_DIMENSIONS.z
                );

                let true_center = (max + min) / 2.0;
                let true_dimensions = Vector3D::new(selection_box.aabb.dimensions.x as f32 * TILE_DIMENSIONS.x,
                    selection_box.aabb.dimensions.y as f32 * TILE_DIMENSIONS.y,
                    selection_box.aabb.dimensions.z as f32 * TILE_DIMENSIONS.z 
                );

                for i in 0..3 { 

                    let mut verts: Vector3Array = Vector3Array::new();  
                    let mut normals: Vector3Array = Vector3Array::new();
                    let mut uvs: Vector2Array = Vector2Array::new();

                    let smaller_x = Float::min(1.0, true_dimensions.y /2.0);
                    let smaller_y = Float::min(1.0, true_dimensions.y /2.0);
                    let smaller_z = Float::min(1.0, true_dimensions.y /2.0);

                    let margin = Float::min(smaller_x, Float::min(smaller_y, smaller_z));

                    match i {
                        0 => { // top and bottom
                            
                            //store vectors as nalgebra's Vector3 to do transformations
                            let mut pts: Vec<Vector3D> = Vec::new();

                            let top_right = Vector3D::new(max.x , max.y , max.z );
                            let inner_top_right = Vector3D::new( //inner top right
                                max.x  - margin,
                                max.y ,
                                max.z  - margin
                            );

                            pts.push(Vector3D::new(min.x , max.y , max.z )); //0 top left
                            pts.push(top_right); //1
                            pts.push(Vector3D::new( //2 inner top left
                                min.x  + margin,
                                max.y ,
                                max.z  - margin
                            ));
                            pts.push(inner_top_right); //3
                            pts.push(top_right); //4
                            pts.push(Vector3D::new(max.x , max.y , min.z )); //5 bottom right
                            pts.push(inner_top_right); //6
                            pts.push(Vector3D::new( //7 inner bottom right
                                    max.x  - margin,
                                    max.y ,
                                    min.z  + margin
                            ));

                            let mut uv: Vec<Vector2D> = Vec::new();

                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.x , 0.0));
                            uv.push(Vector2D::new(margin, margin));
                            uv.push(Vector2D::new(1.0 * true_dimensions.x  - margin, margin));

                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.z , 0.0));
                            uv.push(Vector2D::new(margin, margin));
                            uv.push(Vector2D::new(1.0 * true_dimensions.z  - margin, margin));

                            for iter in pts.iter().zip(uv.iter()) {
                                let (pt, u) = iter;

                                uvs.push(&Vector2::new(u.x, u.y));
                                verts.push(&Vector3::new(pt.x, pt.y, pt.z));
                            }

                            let pts_len = pts.len();
                            for i in 0..pts_len {

                                let new_pt = pts[i] - true_center;
                                let u = uv[i];

                                let rot = Rotation3::new(Vector3D::y() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                pts.push(rotated_pt);
                                uv.push(u);

                                uvs.push(&Vector2::new(u.x, u.y));
                                verts.push(&Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                            }

                            for n in 0..pts.len() {
                                normals.push(&Vector3::new(0.0, 1.0, 0.0));
                            }

                            for iter in pts.iter().zip(uv.iter()) {
                                let (pt, u) = iter;
                                let new_pt = pt - true_center;

                                let rot = Rotation3::new(Vector3D::x() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                uvs.push(&Vector2::new(u.x, u.y));
                                verts.push(&Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                                normals.push(&Vector3::new(0.0,-1.0,0.0));
                            }

                        },
                        1 => { //left and right

                            let mut pts: Vec<Vector3D> = Vec::new();

                            let top_right = Vector3D::new(max.x , max.y , max.z );
                            let inner_top_right = Vector3D::new( //inner top right
                                max.x ,
                                max.y  - margin,
                                max.z  - margin
                            );

                            pts.push(Vector3D::new(max.x , max.y , min.z )); //0 top left
                            pts.push(top_right); //1
                            pts.push(Vector3D::new( //2 inner top left
                                max.x ,
                                max.y  - margin,
                                min.z  + margin
                            ));
                            pts.push(inner_top_right); //3
                            pts.push(top_right); //4
                            pts.push(Vector3D::new(max.x , min.y , max.z )); //5 bottom right
                            pts.push(inner_top_right); //6
                            pts.push(Vector3D::new( //7 inner bottom right
                                max.x ,
                                min.y  + margin,
                                max.z  - margin
                            ));

                            let mut uv: Vec<Vector2D> = Vec::new();

                            uv.push(Vector2D::new(1.0 * true_dimensions.z , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.z  - margin, margin));
                            uv.push(Vector2D::new(margin, margin));

                            uv.push(Vector2D::new(1.0 * true_dimensions.y , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.y  - margin, margin));
                            uv.push(Vector2D::new(margin, margin));

                            for iter in pts.iter().zip(uv.iter()) {
                                let (pt, u) = iter;

                                uvs.push(&Vector2::new(u.x, u.y));
                                verts.push(&Vector3::new(pt.x, pt.y, pt.z));
                            }

                            let pts_len = pts.len();
                            for i in 0..pts_len {

                                let new_pt = pts[i] - true_center;
                                let u = uv[i];

                                let rot = Rotation3::new(Vector3D::x() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                pts.push(rotated_pt);
                                uv.push(u);

                                uvs.push(&Vector2::new(u.x, u.y));
                                verts.push(&Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                            }

                            for n in 0..pts.len() {
                                normals.push(&Vector3::new(1.0, 0.0, 0.0));
                            }

                            for iter in pts.iter().zip(uv.iter()) {
                                let (pt, u) = iter;
                                let new_pt = pt - true_center;

                                let rot = Rotation3::new(Vector3D::y() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                uvs.push(&Vector2::new(u.x, u.y));
                                verts.push(&Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                                normals.push(&Vector3::new(-1.0,0.0,0.0));
                            }

                        }
                        2 => { //front and back
                            let mut pts: Vec<Vector3D> = Vec::new();

                            let top_right = Vector3D::new(max.x , max.y , min.z );
                            let inner_top_right = Vector3D::new( //inner top right
                                max.x  - margin,
                                max.y  - margin,
                                min.z 
                            );

                            pts.push(Vector3D::new(min.x , max.y , min.z )); //0 top left
                            pts.push(top_right); //1
                            pts.push(Vector3D::new( //2 inner top left
                                min.x  + margin,
                                max.y  - margin,
                                min.z 
                            ));
                            pts.push(inner_top_right); //3
                            pts.push(top_right); //4
                            pts.push(Vector3D::new(max.x , min.y , min.z )); //5 bottom right
                            pts.push(inner_top_right); //6
                            pts.push(Vector3D::new( //7 inner bottom right
                                max.x  - margin,
                                min.y  + margin,
                                min.z 
                            ));

                            let mut uv: Vec<Vector2D> = Vec::new();

                            uv.push(Vector2D::new(1.0 * true_dimensions.x , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.x  - margin, margin));
                            uv.push(Vector2D::new(margin, margin));

                            uv.push(Vector2D::new(1.0 * true_dimensions.y , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.y  - margin, margin));
                            uv.push(Vector2D::new(margin, margin));

                            for iter in pts.iter().zip(uv.iter()) {
                                let (pt, u) = iter;

                                uvs.push(&Vector2::new(u.x, u.y));
                                verts.push(&Vector3::new(pt.x, pt.y, pt.z));
                            }

                            let pts_len = pts.len();
                            for i in 0..pts_len {

                                let new_pt = pts[i] - true_center;
                                let u = uv[i];

                                let rot = Rotation3::new(Vector3D::z() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                pts.push(rotated_pt);
                                uv.push(u);

                                uvs.push(&Vector2::new(u.x, u.y));
                                verts.push(&Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                            }

                            for n in 0..pts.len() {
                                normals.push(&Vector3::new(0.0, 0.0, 1.0));
                            }

                            for iter in pts.iter().zip(uv.iter()) {
                                let (pt, u) = iter;
                                let new_pt = pt - true_center;

                                let rot = Rotation3::new(Vector3D::y() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                uvs.push(&Vector2::new(u.x, u.y));
                                verts.push(&Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                                normals.push(&Vector3::new(0.0,0.0,-1.0));
                            }
                        },
                        _ => {}
                    } 

                    

                    let mut indices: Int32Array = Int32Array::new();

                    //add indices for all "quads" in the face;
                    for j in 0..8 {
                        let k = offset + j*4;

                        indices.push(k+2);
                        indices.push(k+1);
                        indices.push(k);

                        indices.push(k+2);
                        indices.push(k+3);
                        indices.push(k+1);

                    }

                    mesh_data.verts.push_array(&verts);
                    mesh_data.normals.push_array(&normals);
                    mesh_data.uvs.push_array(&uvs);
                    mesh_data.indices.push_array(&indices);

                    //increase the offset for the next loop by the number of verts in the face
                    offset += verts.len() as i32;
                }

                // godot_print!("Updated selection box mesh");
                
            }

        })
    
}
    