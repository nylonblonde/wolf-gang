
use std::collections::HashMap;
use gdnative::prelude::*;
use gdnative::api::{
    GeometryInstance,
    ImmediateGeometry,
    Mesh,
};

use crate::node;

use legion::prelude::*;
use crate::node::NodeName;  

pub struct MeshData {
    pub verts: Vector3Array,
    pub uvs: Vector2Array,
    pub uv2s: Vector2Array,
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
            uv2s: Vector2Array::new(),
            normals: Vector3Array::new(),
            indices: Int32Array::new()
        }
    }
}

pub fn create_tag_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("custom_mesh_system")
        .read_component::<Material>()
        .with_query(<Read<MeshData>>::query()
            .filter(!tag::<node::NodeName>())
        )
        .build(move |commands, world, _, query|{
            for (entity, _) in query.iter_entities(&mut *world) {
                commands.exec_mut(move |world: &mut World| {
                    let immediate_geometry = ImmediateGeometry::new();

                    let node_name = unsafe { node::add_node(immediate_geometry.upcast()) }.unwrap();
                    match world.add_tag(entity, node_name){
                        Ok(_) => {},
                        Err(_) => godot_print!("Couldn't add tag!")
                    }
                });
            }
        })
}

pub fn create_draw_system_local() -> Box<dyn Runnable> {
    SystemBuilder::new("custom_mesh_system")
        .read_component::<Material>()
        .with_query(<(Read<MeshData>, Tagged<NodeName>)>::query()
            .filter(changed::<MeshData>())
        )
        .build_thread_local(move |_, world, _, query|{

            let mut entities: HashMap<Entity, &ImmediateGeometry> = HashMap::new();

            for (entity, (mesh_data, mesh_name)) in query.iter_entities(&mut *world) {

                let verts = &mesh_data.verts;
                let uvs = &mesh_data.uvs;
                let uv2s = &mesh_data.uv2s;
                let normals = &mesh_data.normals;
                let indices = &mesh_data.indices;
                
                let immediate_geometry: Option<Ref<ImmediateGeometry>> = unsafe { 
                    match node::get_node(&crate::OWNER_NODE.as_ref().unwrap().assume_safe(), mesh_name.0.clone()) {
                        Some(r) => {
                            Some(r.assume_safe().cast::<ImmediateGeometry>().unwrap().assume_shared())
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

                unsafe {

                    let immediate_geometry = immediate_geometry.unwrap().assume_safe();
            
                    entities.insert(entity, immediate_geometry.as_ref());

                    immediate_geometry.clear();
                    immediate_geometry.begin(Mesh::PRIMITIVE_TRIANGLES, Null::null());
                    
                    let uv2s_len = uv2s.len();

                    for i in 0..indices.len() {
                        let index = indices.get(i);

                        immediate_geometry.set_normal(normals.get(index));
                        immediate_geometry.set_uv(uvs.get(index));
                        if index < uv2s_len {
                            immediate_geometry.set_uv2(uv2s.get(index));
                        }
                        immediate_geometry.add_vertex(verts.get(index));
                    }

                    immediate_geometry.end();
                }
                
                godot_print!("Draw only once")
            }

            for (entity, immediate_geometry) in entities {
                match world.get_component::<Material>(entity) {
                    Some(r) => {
                        let resource = ResourceLoader::godot_singleton().load(GodotString::from_str(match r.name {
                            Some(r) => r,
                            None => { 
                                //TODO: make it so it grabs a default material if no name value is set.
                                panic!("Material name returned None");
                            }
                        }), GodotString::from_str("Material"), false);
            
                        immediate_geometry.upcast::<GeometryInstance>().set_material_override(match resource {
                                Some(r) => r,
                                None => {
                                    //TODO: Same thing, gotta get a default material if none is found
                                    panic!("Resource {:?} does not exist", r.name);
                                }
                            }
                            .cast::<gdnative::api::Material>().unwrap()
                        );
                    }, 
                    None => {
                        godot_print!("No material found");
                    }
                };
            }

        })
    
}

