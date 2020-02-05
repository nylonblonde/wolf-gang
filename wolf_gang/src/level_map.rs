use crate::Point;
use crate::custom_mesh; 
use crate::geometry::aabb::Int32AABB;

use gdnative::*;

use legion::prelude::*;

use std::collections::HashMap;

pub fn create_system() -> Box<dyn Schedulable> {
    SystemBuilder::<()>::new("map_system")
            .with_query(<(Read<MapChunkData>, Write<custom_mesh::MeshData>)>::query()
                .filter(changed::<MapChunkData>())
            )
            .build(move |commands, world, resource, queries| {
                for (entity, (map_data, mut mesh_data)) in queries.iter_entities(&mut *world) {
                    godot_print!("{:?}", "there should only be one tick");
                    mesh_data.verts.push(&Vector3::new(1.0,0.0,0.0));
                    mesh_data.verts.push(&Vector3::new(0.0,0.0,0.0));
                    mesh_data.verts.push(&Vector3::new(1.0,0.0,1.0));
                    mesh_data.verts.push(&Vector3::new(0.0,0.0,1.0));

                    mesh_data.normals.push(&Vector3::new(0.0,1.0,0.0));
                    mesh_data.normals.push(&Vector3::new(0.0,1.0,0.0));
                    mesh_data.normals.push(&Vector3::new(0.0,1.0,0.0));
                    mesh_data.normals.push(&Vector3::new(0.0,1.0,0.0));

                    mesh_data.uvs.push(&Vector2::new(0.0,0.0));
                    mesh_data.uvs.push(&Vector2::new(0.0,0.0));
                    mesh_data.uvs.push(&Vector2::new(0.0,0.0));
                    mesh_data.uvs.push(&Vector2::new(0.0,0.0));

                    mesh_data.indices.push(2);
                    mesh_data.indices.push(1);
                    mesh_data.indices.push(0);
                    mesh_data.indices.push(2);
                    mesh_data.indices.push(3);
                    mesh_data.indices.push(1);
                }
            }
        )
} 

pub struct Map {
    chunk_dimensions: Int32AABB,
    map_chunk_pool: HashMap<Point, MapChunkData>
}

impl Map {
    pub fn new() -> Self {
        Map { 
            map_chunk_pool: HashMap::new(),
            chunk_dimensions: Int32AABB::new(Point(0,0,0), Point(10,10,10))
        }
    }

    pub fn local_process(world: &mut legion::world::World, owner: &mut Node) {
        let query = <(
            Read<custom_mesh::MeshData>,
            Write<MapMesh>,
        )>::query()
            .filter(changed::<custom_mesh::MeshData>());
    
        for (mesh_data, mut map_mesh) in query.iter(world) {
    
            let verts = &mesh_data.verts;
            let uvs = &mesh_data.uvs;
            let normals = &mesh_data.normals;
            let indices = &mesh_data.indices;
    
            let mut arr = VariantArray::new();
    
            let mut mesh_instance = match &map_mesh.name {
                Some(r) => {
                    unsafe { let mesh_instance: MeshInstance = owner.find_node(GodotString::from_str(r), false, true)
                        .unwrap()
                        .cast().
                        unwrap();
                
                        mesh_instance
                    }
                },
                None => {
                    unsafe {
                        let mesh_instance = MeshInstance::new();
                        let name = mesh_instance.get_name().to_string();
                        map_mesh.name = Some(name);
                        owner.add_child(Some(mesh_instance.to_node()), true); 

                        godot_print!("name: {}", mesh_instance.get_name().to_string());

                        mesh_instance
                    }
                }
            };
    
            //resize to the expected size for meshes
            arr.resize(Mesh::ARRAY_MAX as i32);
    
            //create an ArrayMesh which we will feed the VariantArray with surface_from_arrays
            let mut array_mesh = ArrayMesh::new();
    
            arr.set(Mesh::ARRAY_VERTEX as i32, &Variant::from_vector3_array(verts));
            arr.set(Mesh::ARRAY_TEX_UV as i32, &Variant::from_vector2_array(uvs));
            arr.set(Mesh::ARRAY_NORMAL as i32, &Variant::from_vector3_array(normals));
            arr.set(Mesh::ARRAY_INDEX as i32, &Variant::from_int32_array(indices));
    
            array_mesh.add_surface_from_arrays(
                Mesh::PRIMITIVE_TRIANGLES, 
                arr, 
                VariantArray::new(), 
                Mesh::ARRAY_COMPRESS_DEFAULT
            );
    
            unsafe { 
                mesh_instance.set_mesh(Some(array_mesh.to_mesh()));
            }
    
            godot_print!("Draw only once")
        }
    }
}

pub struct MapMesh {
    name: Option<String>,
}

impl MapMesh {
    pub fn new() -> Self {
        MapMesh {
            name: None,
        }
    }
}

//TODO: evaluate whether MapChunkData should be stored in a vec or octree
pub struct MapChunkData {
    tiles: Vec<Vector3>,
}

impl MapChunkData {
    pub fn new() -> Self {
        MapChunkData {
            tiles: Vec::<Vector3>::new(),
        }
    }
}

