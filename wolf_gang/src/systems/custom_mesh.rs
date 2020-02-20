
use gdnative::{
    ArrayMesh,
    godot_print, 
    GodotString, 
    Int32Array, 
    Mesh,
    MeshInstance,
    Node,
    ResourceLoader,
    Variant,
    VariantArray, 
    Vector2Array, 
    Vector3Array
};

use crate::node;

use legion::prelude::*;
use crate::node::NodeName;
use std::collections::HashMap;

pub struct MeshInstancePool {
    pool: HashMap<GodotString, MeshInstance>,
}

impl MeshInstancePool {
    pub fn new() -> Self {
        MeshInstancePool {
            pool: HashMap::new()
        }
    }
}

pub struct MeshData {
    pub verts: Vector3Array,
    pub uvs: Vector2Array,
    pub normals: Vector3Array,
    pub indices: Int32Array,
}

pub struct Material {
    name: Option<&'static str>,
}

impl Material {
    pub fn new() -> Self {
        Material {
            name: None
        }
    }

    pub fn from_str(s: &'static str) -> Self {
        Material {
            name: Some(s)
        }
    }
}

impl MeshData {
    pub fn new() -> Self {
        MeshData {
            verts: Vector3Array::new(),
            uvs: Vector2Array::new(),
            normals: Vector3Array::new(),
            indices: Int32Array::new()
        }
    }
}

pub fn create_system_local() -> Box<dyn Runnable> {
    SystemBuilder::new("custom_mesh_system")
        .read_component::<Material>()
        .with_query(<(Read<MeshData>, Tagged<NodeName>)>::query()
            .filter(changed::<MeshData>())
        )
        .build_thread_local(move |commands, world, resource, query|{
            for (entity, (mesh_data, mut mesh_name)) in query.iter_entities(&mut *world) {

                let verts = &mesh_data.verts;
                let uvs = &mesh_data.uvs;
                let normals = &mesh_data.normals;
                let indices = &mesh_data.indices;
        
                let mut arr = VariantArray::new();
        
                let mut mesh_instance: Option<MeshInstance> = None;
                
                unsafe { 
                    mesh_instance = match node::find_node(GodotString::from(mesh_name.0.clone())) {
                        Some(r) => {
                            Some(r.cast().unwrap())
                        },
                        None => {
                            godot_print!("Couldn't find mesh instance");
                            None
                        }
                    };
                }

                if mesh_instance.is_none() {
                    continue;
                }

                let mut mesh_instance = mesh_instance.unwrap();

                // let mut mesh_instance = match &mesh_name.name {
                //     Some(r) => {
                //         unsafe { 
                //             let mesh_instance: MeshInstance = crate::node::find_node(GodotString::from_str(r))
                //             .unwrap()
                //             .cast()
                //             .unwrap();
                    
                //             mesh_instance
                //         }
                //     },
                //     None => {
                //         unsafe {
                //             let mesh_instance = MeshInstance::new();
                //             let name = mesh_instance.get_name().to_string();
                //             mesh_name.name = Some(name);

                //             node::add_node(&mut mesh_instance.to_node(), Some(&mut mesh_name));
        
                //             godot_print!("name: {:?}", mesh_name.name);
        
                //             mesh_instance
                //         }
                //     }
                // };
        
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

                match world.get_component::<Material>(entity) {
                    Some(r) => {
                        unsafe {
                            let resource = ResourceLoader::godot_singleton().load(GodotString::from_str(match r.name {
                                Some(r) => r,
                                None => { 
                                    //TODO: make it so it grabs a default material if no name value is set.
                                    panic!("Material name returned None");
                                }
                            }), GodotString::from_str("Material"), false);
                
                            mesh_instance.set_surface_material(0, Some(match resource {
                                    Some(r) => r,
                                    None => {
                                        //TODO: Same thing, gotta get a default material if none is found
                                        panic!("Resource {:?} does not exist", r.name);
                                    }
                                }
                                .cast::<gdnative::Material>().unwrap())
                            );
                        }
                    }, 
                    None => {
                        godot_print!("No material found");
                    }
                };
                
                godot_print!("Draw only once")
            }
        })
    
}

