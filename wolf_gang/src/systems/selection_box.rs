use gdnative::{godot_print, GodotString, Input, Int32Array, Vector2, Vector2Array, Vector3, Vector3Array};
use legion::prelude::*;
use nalgebra::{Rotation2, Rotation3};
use num::Float;

use crate::geometry::aabb;
use crate::custom_mesh;
use crate::node;
use crate::level_map::TILE_DIMENSIONS;
use crate::input;

type AABB = aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;

type Vector3D = nalgebra::Vector3<f32>;
type Vector2D = nalgebra::Vector2<f32>;

pub struct SelectionBox {
    aabb: AABB
}

impl SelectionBox {
    ///Creates a SelectionBox with an aabb at center (0,0,0) with dimensions of (1,1,1).
    pub fn new() -> Self {
        SelectionBox {
            aabb: AABB::new(Point::new(0,0,0), Point::new(1,1,1))
        }
    }

    pub fn from_aabb(aabb: AABB) -> Self {
        SelectionBox {
            aabb
        }
    }
}

pub fn create_system_local() -> Box<dyn Runnable> {
    SystemBuilder::<()>::new("selection_box_local_system")
        .with_query(<(Read<SelectionBox>, Write<node::NodeName>, )>::query()
            .filter(changed::<SelectionBox>(),),
        )
        .build_thread_local(move |commands, world, resource, queries|{
            for(entity, (selection_box, node_name)) in queries.iter_entities(&mut *world) {

            }
        })
}

pub fn create_thread_local_fn() -> Box<dyn FnMut(&mut World)> {
    Box::new(|world: &mut World|{

        let move_forward = input::Action("move_forward".to_string());

        let input_query = <(Read<input::InputComponent>, Tagged<input::Action>)>::query()
            .filter(changed::<input::InputComponent>())
            .filter(tag_value(&move_forward));

        for (input_component, tag) in input_query.iter(world) {

            if tag.0 == move_forward.0 {
                if input_component.repeated(0.5) {
                    godot_print!("Uppies");
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

            for (entity, (selection_box, mut mesh_data)) in queries.iter_entities(&mut *world) {

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

                godot_print!("{:?}", selection_box.aabb.dimensions);
                godot_print!("{:?}",true_center);

                for i in 0..3 { 

                    let mut verts: Vector3Array = Vector3Array::new();  
                    let mut normals: Vector3Array = Vector3Array::new();
                    let mut uvs: Vector2Array = Vector2Array::new();

                    let smaller_x = Float::min(1.0, true_dimensions.x /2.0);
                    let smaller_y = Float::min(1.0, true_dimensions.y /2.0);
                    let smaller_z = Float::min(1.0, true_dimensions.z /2.0);

                    match i {
                        0 => { // top and bottom
                            
                            //store vectors as nalgebra's Vector3 to do transformations
                            let mut pts: Vec<Vector3D> = Vec::new();

                            let top_right = Vector3D::new(max.x , max.y , max.z );
                            let inner_top_right = Vector3D::new( //inner top right
                                max.x  - smaller_x,
                                max.y ,
                                max.z  - smaller_z
                            );

                            pts.push(Vector3D::new(min.x , max.y , max.z )); //0 top left
                            pts.push(top_right); //1
                            pts.push(Vector3D::new( //2 inner top left
                                min.x  + smaller_x,
                                max.y ,
                                max.z  - smaller_z
                            ));
                            pts.push(inner_top_right); //3
                            pts.push(top_right); //4
                            pts.push(Vector3D::new(max.x , max.y , min.z )); //5 bottom right
                            pts.push(inner_top_right); //6
                            pts.push(Vector3D::new( //7 inner bottom right
                                    max.x  - smaller_x,
                                    max.y ,
                                    min.z  + smaller_z
                            ));

                            let mut uv: Vec<Vector2D> = Vec::new();

                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.x , 0.0));
                            uv.push(Vector2D::new(smaller_x, smaller_z));
                            uv.push(Vector2D::new(1.0 * true_dimensions.x  - smaller_x, smaller_z));

                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.z , 0.0));
                            uv.push(Vector2D::new(smaller_z, smaller_x));
                            uv.push(Vector2D::new(1.0 * true_dimensions.z  - smaller_z, smaller_x));

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
                                max.y  - smaller_y,
                                max.z  - smaller_z
                            );

                            pts.push(Vector3D::new(max.x , max.y , min.z )); //0 top left
                            pts.push(top_right); //1
                            pts.push(Vector3D::new( //2 inner top left
                                max.x ,
                                max.y  - smaller_y,
                                min.z  + smaller_z
                            ));
                            pts.push(inner_top_right); //3
                            pts.push(top_right); //4
                            pts.push(Vector3D::new(max.x , min.y , max.z )); //5 bottom right
                            pts.push(inner_top_right); //6
                            pts.push(Vector3D::new( //7 inner bottom right
                                max.x ,
                                min.y  + smaller_y,
                                max.z  - smaller_z
                            ));

                            let mut uv: Vec<Vector2D> = Vec::new();

                            uv.push(Vector2D::new(1.0 * true_dimensions.z , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.z  - smaller_z, smaller_y));
                            uv.push(Vector2D::new(smaller_z, smaller_y));

                            uv.push(Vector2D::new(1.0 * true_dimensions.y , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.y  - smaller_y, smaller_z));
                            uv.push(Vector2D::new(smaller_y, smaller_z));

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
                                max.x  - smaller_x,
                                max.y  - smaller_y,
                                min.z 
                            );

                            pts.push(Vector3D::new(min.x , max.y , min.z )); //0 top left
                            pts.push(top_right); //1
                            pts.push(Vector3D::new( //2 inner top left
                                min.x  + smaller_x,
                                max.y  - smaller_y,
                                min.z 
                            ));
                            pts.push(inner_top_right); //3
                            pts.push(top_right); //4
                            pts.push(Vector3D::new(max.x , min.y , min.z )); //5 bottom right
                            pts.push(inner_top_right); //6
                            pts.push(Vector3D::new( //7 inner bottom right
                                max.x  - smaller_x,
                                min.y  + smaller_y,
                                min.z 
                            ));

                            let mut uv: Vec<Vector2D> = Vec::new();

                            uv.push(Vector2D::new(1.0 * true_dimensions.x , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.x  - smaller_x, smaller_y));
                            uv.push(Vector2D::new(smaller_x, smaller_y));

                            uv.push(Vector2D::new(1.0 * true_dimensions.y , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * true_dimensions.y  - smaller_y, smaller_x));
                            uv.push(Vector2D::new(smaller_y, smaller_x));

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

                godot_print!("Updated selection box mesh");
                
            }

        })
    
}
    