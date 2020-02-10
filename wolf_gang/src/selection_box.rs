use gdnative::{godot_print, Int32Array, Vector2, Vector2Array, Vector3, Vector3Array};
use legion::prelude::*;

use crate::geometry::aabb;
use crate::custom_mesh;

type AABB = aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;

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

pub fn create_system() -> Box<dyn Schedulable> {
    
    SystemBuilder::<()>::new("selection_box_system")
        .with_query(<(Read<SelectionBox>, Write<custom_mesh::MeshData>,)>::query()
            .filter(changed::<SelectionBox>(),)
        )
        .build(move |commands, world, resource, queries| {
            
            for (entity, (selection_box, mut mesh_data)) in queries.iter_entities(&mut *world) {

                let mut verts: Vector3Array = Vector3Array::new();  
                
                verts.push(&Vector3::new(0.0, 0.0, 1.0));
                verts.push(&Vector3::new(1.0, 0.0, 1.0));
                verts.push(&Vector3::new(0.0, 0.0, 0.0));
                verts.push(&Vector3::new(1.0, 0.0, 0.0));
                
                let mut normals: Vector3Array = Vector3Array::new();

                normals.push(&Vector3::new(0.0, 1.0, 0.0));
                normals.push(&Vector3::new(0.0, 1.0, 0.0));
                normals.push(&Vector3::new(0.0, 1.0, 0.0));
                normals.push(&Vector3::new(0.0, 1.0, 0.0));

                let mut uvs: Vector2Array = Vector2Array::new();
                uvs.push(&Vector2::new(0.0, 0.0));
                uvs.push(&Vector2::new(1.0, 0.0));
                uvs.push(&Vector2::new(0.0, 1.0));
                uvs.push(&Vector2::new(1.0, 1.0));

                let mut indices: Int32Array = Int32Array::new();
                indices.push(2);
                indices.push(1);
                indices.push(0);

                indices.push(3);
                indices.push(1);
                indices.push(2);

                mesh_data.verts.push_array(&verts);
                mesh_data.normals.push_array(&normals);
                mesh_data.uvs.push_array(&uvs);
                mesh_data.indices.push_array(&indices);

                godot_print!("Updated selection box mesh");
                
            }

        })
    
}
    