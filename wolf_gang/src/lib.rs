use gdnative::*;

use legion::prelude::*;
use lazy_static::*;

use std::collections::HashMap;

mod collections;
mod geometry;

mod level_map;
mod custom_mesh;
mod selection_box;

#[cfg(test)]
mod tests;

/// The WolfGang "class"
#[derive(NativeClass)]
#[inherit(Node)]
#[user_data(user_data::LocalCellData<WolfGang>)]
pub struct WolfGang {
    universe: Option<Universe>,
    world: Option<legion::world::World>,
    schedule: Option<Schedule>,
}

use std::sync::Mutex;

static mut OWNER_NODE: Option<Mutex<Node>> = None;

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl WolfGang {
    
    /// The "constructor" of the class.
    fn _init(owner: Node) -> Self {
        unsafe { OWNER_NODE = Some(Mutex::new(owner)); }

        WolfGang {
            universe: None,
            world: None,
            schedule: None,
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
            (0..1).map(|_| (level_map::MapChunkData::new(), custom_mesh::MeshName::new(), custom_mesh::MeshData::new(),))
        );

        world.insert(
            (),
            (0..1).map(|_| (level_map::Map::new(),))
        );

        world.insert(
            (),
            (0..1).map(|_| (selection_box::SelectionBox::new(), custom_mesh::MeshData::new(), custom_mesh::MeshName::new(), custom_mesh::Material::from_str("res://select_box.material"),))
        );

        // self.executor = Some(Executor::new(systems));
        let schedule = Schedule::builder()
            .add_system(level_map::create_system())
            .add_system(selection_box::create_system())
            .add_thread_local(custom_mesh::create_system())
            .build();

        self.schedule = Some(schedule);
    }

    #[export]
    fn _process(&mut self, mut owner: Node, delta: f64) {
        // let executor = self.executor.as_mut().unwrap();

        let world = self.world.as_mut().unwrap();
        

        let schedule = self.schedule.as_mut().unwrap();
        schedule.execute(world);

        // executor.execute(world);

        // custom_mesh::local_process(world, &mut owner);

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