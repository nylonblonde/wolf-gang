
use gdnative::{
    GeometryInstance,
    godot_print, 
    GodotString, 
    ImmediateGeometry,
    Int32Array, 
    Mesh,
    ResourceLoader, 
    Vector2Array, 
    Vector3Array
};

use crate::node;

use legion::prelude::*;
use crate::node::NodeName;  

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
        .build_thread_local(move |_, world, _, query|{
            for (entity, (mesh_data, mesh_name)) in query.iter_entities(&mut *world) {

                let verts = &mesh_data.verts;
                let uvs = &mesh_data.uvs;
                let normals = &mesh_data.normals;
                let indices = &mesh_data.indices;
        
                // let mut arr = VariantArray::new();
        
                let immediate_geometry: Option<ImmediateGeometry> = unsafe { 
                    match node::find_node(mesh_name.0.clone()) {
                        Some(r) => {
                            Some(r.cast().unwrap())
                        },
                        None => {
                            godot_print!("Couldn't find mesh instance");
                            None
                        }
                    }
                };

                if immediate_geometry.is_none() {
                    continue;
                }

                let mut immediate_geometry = immediate_geometry.unwrap();
        
                // let mut immediate_geometry = ImmediateGeometry::new();

                unsafe {
                    immediate_geometry.clear();
                    immediate_geometry.begin(Mesh::PRIMITIVE_TRIANGLES, None);
                    
                    for i in 0..indices.len() {
                        let index = indices.get(i);

                        immediate_geometry.set_normal(normals.get(index));
                        immediate_geometry.set_uv(uvs.get(index));
                        immediate_geometry.add_vertex(verts.get(index));
                    }

                    immediate_geometry.end();
                }

                // //resize to the expected size for meshes
                // arr.resize(Mesh::ARRAY_MAX as i32);
        
                //create an ArrayMesh which we will feed the VariantArray with surface_from_arrays
                // let mut array_mesh = ArrayMesh::new();
        
                // arr.set(Mesh::ARRAY_VERTEX as i32, &Variant::from_vector3_array(verts));
                // arr.set(Mesh::ARRAY_TEX_UV as i32, &Variant::from_vector2_array(uvs));
                // arr.set(Mesh::ARRAY_NORMAL as i32, &Variant::from_vector3_array(normals));
                // arr.set(Mesh::ARRAY_INDEX as i32, &Variant::from_int32_array(indices));

                // array_mesh.add_surface_from_arrays(
                //     Mesh::PRIMITIVE_TRIANGLES, 
                //     arr, 
                //     VariantArray::new(), 
                //     Mesh::ARRAY_COMPRESS_DEFAULT
                // );
        
                // unsafe { 
                //     mesh_instance.set_mesh(Some(array_mesh.to_mesh()));
                // }

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
                
                            immediate_geometry.cast::<GeometryInstance>().unwrap().set_material_override(Some(match resource {
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

