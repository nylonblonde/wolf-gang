use gdnative::{godot_print, Int32Array, Vector2, Vector2Array, Vector3, Vector3Array};
use legion::prelude::*;
use nalgebra::{Rotation2, Rotation3};
use num::Float;

use crate::geometry::aabb;
use crate::custom_mesh;

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
            aabb: AABB::new(Point::new(0,0,0), Point::new(3,1,4))
        }
    }

    pub fn from_aabb(aabb: AABB) -> Self {
        SelectionBox {
            aabb
        }
    }
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

                let true_center = (Vector3D::new(max.x as f32, max.y as f32, max.z as f32) + Vector3D::new(min.x as f32, min.y as f32, min.z as f32)) / 2.0;

                godot_print!("{:?}", selection_box.aabb.dimensions);
                godot_print!("{:?}",true_center);

                for i in 0..1 { 
                    // godot_print!("{}", i);

                    //Using nalgebra to rotate the vertices, so gotta to store them in their type first
                    // let points: Vec<nalgebra::Vector3<f32>> = vec![
                    //     nalgebra::Vector3::new(-0.5, 0.5, 0.5),
                    //     nalgebra::Vector3::new(0.5, 0.5, 0.5),
                    //     nalgebra::Vector3::new(0.0, 0.5, 0.0),
                    //     nalgebra::Vector3::new(-0.5, 0.5, -0.5),
                    //     nalgebra::Vector3::new(0.5, 0.5, -0.5),
                    // ];

                    let mut verts: Vector3Array = Vector3Array::new();  
                    let mut normals: Vector3Array = Vector3Array::new();
                    let mut uvs: Vector2Array = Vector2Array::new();

                    //Fill the Vector3Array with Godot's Vector3 type that it needs after rotation is done
                    // for mut pt in points {

                        // match i { //Right face
                        //     1 => {
                        //         let rot = Rotation3::from_euler_angles(0.0, std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
                        //         pt = rot.transform_vector(&pt);
                        //     },
                        //     2 => { //Back face
                        //         let rot = Rotation3::from_euler_angles(-std::f32::consts::FRAC_PI_2, 0.0, 0.0);
                        //         pt = rot.transform_vector(&pt);
                        //     },
                        //     3 => { //Left face
                        //         let rot = Rotation3::from_euler_angles(0.0, -std::f32::consts::FRAC_PI_2, -std::f32::consts::FRAC_PI_2);
                        //         pt = rot.transform_vector(&pt);
                        //     }
                        //     4 => { //Front face
                        //         let rot = Rotation3::from_euler_angles(-std::f32::consts::FRAC_PI_2, std::f32::consts::PI, 0.0);
                        //         pt = rot.transform_vector(&pt);
                        //     }
                        //     5 => { //Bottom face
                        //         let rot = Rotation3::from_euler_angles(0.0, 0.0, std::f32::consts::PI);
                        //         pt = rot.transform_vector(&pt);
                        //     }
                        //     _=> {}
                        // }

                        // verts.push(&Vector3::new(pt.x, pt.y, pt.z));
                    // }

                    let smaller_x = Float::min(1.0, selection_box.aabb.dimensions.x as f32/2.0);
                    let smaller_y = Float::min(1.0, selection_box.aabb.dimensions.y as f32/2.0);
                    let smaller_z = Float::min(1.0, selection_box.aabb.dimensions.z as f32/2.0);

                    match i {
                        0 => { // top and bottom
                            
                            //store vectors as nalgebra's Vector3 to do transformations
                            let mut pts: Vec<Vector3D> = Vec::new();

                            let top_right = Vector3D::new(max.x as f32, max.y as f32, max.z as f32);
                            let inner_top_right = Vector3D::new( //inner top right
                                max.x as f32 - smaller_x,
                                max.y as f32,
                                max.z as f32 - smaller_z
                            );

                            pts.push(Vector3D::new(min.x as f32, max.y as f32, max.z as f32)); //0 top left
                            godot_print!("{:?}", pts.last());
                            pts.push(top_right); //1
                            pts.push(Vector3D::new( //2 inner top left
                                min.x as f32 + smaller_x,
                                max.y as f32,
                                max.z as f32 - smaller_z
                            ));
                            pts.push(inner_top_right); //3
                            pts.push(top_right); //4
                            pts.push(Vector3D::new(max.x as f32, max.y as f32, min.z as f32)); //5 bottom right
                            pts.push(inner_top_right); //6
                            pts.push(Vector3D::new( //7 inner bottom right
                                    max.x as f32 - smaller_x,
                                    max.y as f32,
                                    min.z as f32 + smaller_z
                            ));

                            let mut uv: Vec<Vector2D> = Vec::new();

                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * selection_box.aabb.dimensions.x as f32, 0.0));
                            uv.push(Vector2D::new(smaller_x, smaller_z));
                            uv.push(Vector2D::new(1.0 * selection_box.aabb.dimensions.x as f32 - smaller_x, smaller_z));

                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * selection_box.aabb.dimensions.z as f32, 0.0));
                            uv.push(Vector2D::new(smaller_z, smaller_x));
                            uv.push(Vector2D::new(1.0 * selection_box.aabb.dimensions.z as f32 - smaller_z, smaller_x));

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

                            for iter in pts.iter().zip(uv.iter()) {
                                let (pt, u) = iter;
                                let new_pt = pt - true_center;

                                let rot = Rotation3::new(Vector3D::x() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                uvs.push(&Vector2::new(u.x, u.y));
                                verts.push(&Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                            }

                            // verts.push(&Vector3::new(min.x as f32, max.y as f32, max.z as f32)); //top left
                            // verts.push(&Vector3::new(max.x as f32, max.y as f32, max.z as f32)); //top right
                            // verts.push(&Vector3::new( //inner top left
                            //     min.x as f32 + smaller_x,
                            //     max.y as f32,
                            //     max.z as f32 - smaller_z
                            // ));
                            // verts.push(&Vector3::new( //inner top right
                            //     max.x as f32 - smaller_x,
                            //     max.y as f32,
                            //     max.z as f32 - smaller_z
                            // ));
                            // verts.push(&Vector3::new(min.x as f32, max.y as f32, min.z as f32)); //bottom left
                            // verts.push(&Vector3::new(max.x as f32, max.y as f32, min.z as f32)); //bottom right
                            // verts.push(&Vector3::new(
                            //     min.x as f32 + smaller_x,
                            //     max.y as f32,
                            //     min.z as f32 + smaller_z
                            // ));
                            // verts.push(&Vector3::new(
                            //     max.x as f32 - smaller_x,
                            //     max.y as f32,
                            //     min.z as f32 + smaller_z
                            // ));
                        

                            for n in 0..verts.len() {
                                normals.push(&Vector3::new(0.0, 1.0, 0.0));
                                // uvs.push(&Vector2::new(0.0,0.0));
                            }

                            // uvs.push(&Vector2::new(0.0, 0.0)); //top left outer
                            // uvs.push(&Vector2::new(1.0 * selection_box.aabb.dimensions.x as f32, 0.0)); //top right outer
                            // uvs.push(&Vector2::new(1.0 * selection_box.aabb.dimensions.x as f32, 0.0)); //bottom left outer
                            // uvs.push(&Vector2::new(0.0, 0.0)); //bottom right outer

                            // uvs.push(&Vector2::new(1.0 * smaller_x, 1.0)); //top left inner
                            // uvs.push(&Vector2::new(1.0, 0.0)); 
                            // uvs.push(&Vector2::new(0.0, 1.0));
                            // uvs.push(&Vector2::new(1.0, 1.0));

                            // uvs.push(&Vector2::new(0.0, 0.0)); //top left outer
                            // uvs.push(&Vector2::new(1.0 / selection_box.aabb.dimensions.x as f32, 0.0)); //top right outer
                            // uvs.push(&Vector2::new(1.0 / selection_box.aabb.dimensions.x as f32, 0.0)); //bottom left outer
                            // uvs.push(&Vector2::new(0.0, 0.0)); //bottom right outer

                            // uvs.push(&Vector2::new(1.0 * smaller_x, 0.0)); //top left inner
                            // uvs.push(&Vector2::new(1.0, 0.0)); 
                            // uvs.push(&Vector2::new(0.0, 1.0));
                            // uvs.push(&Vector2::new(1.0, 1.0));

                        },
                        _ => {}
                    } 

                    

                    let mut indices: Int32Array = Int32Array::new();

                    //add indices for all "quads" in the face;
                    for j in 0..8 {
                        let k = offset + j*4;
                        // godot_print!("k: {}", k);

                        // let k = offset;

                        indices.push(k);
                        indices.push(k+1);
                        indices.push(k+2);

                        indices.push(k+1);
                        indices.push(k+3);
                        indices.push(k+2);

                        // indices.push(k+4);
                        // indices.push(k+5);
                        // indices.push(k+6);

                        // indices.push(k+5);
                        // indices.push(k+7);
                        // indices.push(k+6);
                    }

                    // let j = offset;

                    // indices.push(j+2);
                    // indices.push(j+1);
                    // indices.push(j);

                    // indices.push(j+2);
                    // indices.push(j+4);
                    // indices.push(j+1);

                    // indices.push(j+2);
                    // indices.push(j+3);
                    // indices.push(j+4);

                    // indices.push(j+2);
                    // indices.push(j);
                    // indices.push(j+3);

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
    