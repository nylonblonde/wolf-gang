use gdnative::*;

use legion::prelude::*;

use std::collections::HashMap;

mod collections;
mod geometry;

mod level_map;
mod custom_mesh;

#[derive(Hash, Eq, PartialEq)]
pub struct Point(i32,i32,i32);

/// The WolfGang "class"
#[derive(NativeClass)]
#[inherit(Node)]
#[user_data(user_data::MutexData<WolfGang>)]
pub struct WolfGang {
    universe: Option<Universe>,
    world: Option<legion::world::World>,
    executor: Option<Executor>,
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl WolfGang {
    
    /// The "constructor" of the class.
    fn _init(_owner: Node) -> Self {
        WolfGang {
            universe: None,
            world: None,
            executor: None,
        }
    }
    
    // In order to make a method known to Godot, the #[export] attribute has to be used.
    // In Godot script-classes do not actually inherit the parent class.
    // Instead they are"attached" to the parent object, called the "owner".
    // The owner is passed to every single exposed method.
    #[export]
    fn _ready(&mut self, mut owner: Node) {
        godot_print!("hello, world.");

        self.universe = Some(Universe::new());
        self.world = Some(self.universe.as_ref().unwrap().create_world());

        let world = self.world.as_mut().unwrap();

        world.insert(
            (),
            (0..1).map(|_| (level_map::MapChunkData::new(), level_map::MapMesh::new(), custom_mesh::MeshData::new(),))
        );

        world.insert(
            (),
            (0..1).map(|_| (level_map::Map::new(),))
        );

        let systems = vec![
            level_map::create_system(),
        ];

        self.executor = Some(Executor::new(systems));
        
    }

    #[export]
    fn _process(&mut self, mut owner: Node, delta: f64) {
        let executor = self.executor.as_mut().unwrap();

        let world = self.world.as_mut().unwrap();
        
        executor.execute(world);

        level_map::Map::local_process(world, &mut owner);

    }
}

// Function that registers all exposed classes to Godot
fn init(handle: gdnative::init::InitHandle) {
    handle.add_class::<WolfGang>();
}

// macros that create the entry-points of the dynamic library.
godot_gdnative_init!();
godot_nativescript_init!(init);
godot_gdnative_terminate!();