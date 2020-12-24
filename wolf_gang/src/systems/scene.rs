use gdnative::prelude::*;
use legion::*;

use crate::node;

#[derive(Copy, Clone)]
pub struct InitializeScene {
    path: &'static str
}

impl InitializeScene {
    pub fn new(path: &'static str) -> InitializeScene {
        InitializeScene {
            path
        }
    }
}

pub fn create_scene_init_system() -> impl systems::Runnable {

    SystemBuilder::new("scene_init_system")
        .with_query(<(Entity, Read<InitializeScene>)>::query())
        .build(move |command, world, _, query| {

            for (entity, init_scene) in query.iter(world) {
                println!("Initializing scene {:?}!", init_scene.path);

                let scene = ResourceLoader::godot_singleton().load(init_scene.path, "PackedScene", false).unwrap().cast::<PackedScene>().unwrap();
                unsafe {
                    let scene_instance = scene.assume_safe().instance(0).unwrap().assume_unique();

                    godot_print!("{:?}", scene_instance.name());

                    node::add_node(scene_instance);
                }
                command.remove(*entity);
            }

        })
}
