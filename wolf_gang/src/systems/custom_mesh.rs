
use std::collections::HashMap;
use gdnative::prelude::*;
use gdnative::api::{
    GeometryInstance,
    ImmediateGeometry,
    Mesh,
};

use crate::node;

use legion::*;

pub struct MeshData {
    pub verts: Vec<Vector3>,
    pub uvs: Vec<Vector2>,
    pub uv2s: Vec<Vector2>,
    pub normals: Vec<Vector3>,
    pub indices: Vec<i32>,
}

impl MeshData {
    pub fn clear(&mut self) {
        self.verts.clear();
        self.uvs.clear();
        self.uv2s.clear();
        self.normals.clear();
        self.indices.clear();
    }
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
            verts: Vec::new(),
            uvs: Vec::new(),
            uv2s: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new()
        }
    }
}

pub struct ManuallyChange{}

pub struct RequiresManualChange{}

pub fn create_tag_system() -> impl systems::Runnable {
    SystemBuilder::new("custom_mesh_system")
        .read_component::<Material>()
        .with_query(<Entity>::query()
            .filter(!component::<node::NodeRef>() & component::<MeshData>())
        )
        .build(move |commands, world, _, query|{
            query.for_each(world, |entity| {
                let immediate_geometry = ImmediateGeometry::new();

                let owner = unsafe { crate::OWNER_NODE.as_mut().unwrap().assume_safe() };
                
                let node = unsafe { node::add_node(&owner, immediate_geometry.upcast()) };

                commands.add_component(*entity, node::NodeRef::new(node));
            })
        })
}

pub fn create_draw_system() -> impl systems::Runnable {
    SystemBuilder::new("custom_mesh_system")
        .read_component::<Material>()
        .with_query(<(Entity, Read<MeshData>, Read<node::NodeRef>)>::query()
            .filter(
                (component::<RequiresManualChange>() & component::<ManuallyChange>()) |
                (!component::<RequiresManualChange>() & maybe_changed::<MeshData>())
            )
        )
        .build(move |commands, world, _, query|{

            let mut entities: HashMap<Entity, Ref<ImmediateGeometry>> = HashMap::new();

            query.for_each(world, |(entity, mesh_data, node_ref)| {

                godot_print!("Drawing {:?}", unsafe { node_ref.val().assume_safe().name() });

                let verts = &mesh_data.verts;
                let uvs = &mesh_data.uvs;
                let uv2s = &mesh_data.uv2s;
                let normals = &mesh_data.normals;
                let indices = &mesh_data.indices;
                
                let immediate_geometry: Ref<ImmediateGeometry> = unsafe { 
                    node_ref.val().assume_safe().cast::<ImmediateGeometry>().unwrap().assume_shared()
                };

                entities.insert(*entity, immediate_geometry);

                unsafe {

                    let immediate_geometry = immediate_geometry.assume_safe();            

                    immediate_geometry.clear();
                    immediate_geometry.begin(Mesh::PRIMITIVE_TRIANGLES, Null::null());
                    
                    let uv2s_len = uv2s.len();

                    for index in indices {
                        let index = *index as usize;

                        immediate_geometry.set_normal(normals[index]);
                        immediate_geometry.set_uv(uvs[index]);
                        if index < uv2s_len {
                            immediate_geometry.set_uv2(uv2s[index]);
                        }
                        immediate_geometry.add_vertex(verts[index]);
                    }

                    immediate_geometry.end();
                }
                
            });

            for (entity, immediate_geometry) in entities {

                commands.exec_mut(move |world, _| {
                    if let Some(mut entry) = world.entry(entity) {
                        if let Ok(material) = entry.get_component::<Material>() {
                            let resource = ResourceLoader::godot_singleton().load(match material.name {
                                Some(r) => r,
                                None => { 
                                    //TODO: make it so it grabs a default material if no name value is set.
                                    panic!("Material name returned None");
                                }
                            }, "Material", false);
                
                            unsafe {
                                immediate_geometry.assume_safe().upcast::<GeometryInstance>().set_material_override(match resource {
                                        Some(r) => r,
                                        None => {
                                            //TODO: Same thing, gotta get a default material if none is found
                                            panic!("Resource {:?} does not exist", material.name);
                                        }
                                    }
                                    .cast::<gdnative::api::Material>().unwrap()
                                );
                            }
                        }

                        entry.remove_component::<ManuallyChange>();
                    }
                });
            }

        })
    
}

