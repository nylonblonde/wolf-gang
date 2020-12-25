use gdnative::prelude::*;
use legion::*;

use crate::{
    node,
    node::NodeName,
};

#[derive(Clone)]
pub struct InitializeScene {
    path: String,
}

impl InitializeScene {
    pub fn new(path: String) -> InitializeScene {
        InitializeScene {
            path,
        }
    }
}

pub fn create_scene_init_system() -> Box<dyn FnMut(&mut World, &mut Resources)> {

    let mut query = <(Entity, Read<InitializeScene>)>::query();
    Box::new(move |world, _| {

        let results = query.iter(world)
            .map(|(entity, init_scene)| (*entity, init_scene.clone()))
            .collect::<Vec<(Entity, InitializeScene)>>();

        results.iter().for_each(|(entity, init_scene)| {

            let scene = ResourceLoader::godot_singleton().load(init_scene.path.clone(), "PackedScene", false).unwrap().cast::<PackedScene>().unwrap();
            unsafe {
                let scene_instance = scene.assume_safe().instance(0).unwrap().assume_unique();
                
                let mut needs_node_name: Option<NodeName> = None;

                if let Some(entry) = world.entry(*entity) {

                    match entry.into_component_mut::<NodeName>() {
                        Ok(mut node_name) => {
                            scene_instance.set_name(node_name.clone().0);
                            let new_name = node::add_node(scene_instance).unwrap();
                            node_name.0 = new_name.0;

                        },
                        _ => {
                            needs_node_name = node::add_node(scene_instance);
                        }
                    }
                }

                if let Some(mut entry) = world.entry(*entity) {
                    if let Some(node_name) = needs_node_name {
                        entry.add_component(node_name);
                    }

                    entry.remove_component::<InitializeScene>();
                }

            }
        });

    })
}
