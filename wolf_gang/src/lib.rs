use gdnative::*;

use legion::prelude::*;
use lazy_static::*;

use std::collections::HashMap;

mod collections;
mod geometry;
mod systems;
mod node;

use systems::{camera, input, level_map, custom_mesh, selection_box, transform};

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
static mut DELTA_TIME: f64 = 0.0;

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

        input::InputConfig::from_file(input::CONFIG_PATH);
        godot_print!("hello, world.");


        self.universe = Some(Universe::new());
        self.world = Some(self.universe.as_ref().unwrap().create_world());

        let mut world = self.world.as_mut().unwrap();

        let camera = camera::initialize_camera(&mut world);

        // world.insert(
        //     (),
        //     (0..1).map(|_| (level_map::MapChunkData::new(), custom_mesh::MeshData::new(),))
        // );

        world.insert(
            (),
            (0..1).map(|_| (level_map::Map::new(),))
        );

        selection_box::initialize_selection_box(world, camera);

        // world.insert(
        //     (),
        //     vec![
        //         (
        //             selection_box::SelectionBox::new(), 
        //             custom_mesh::MeshData::new(), 
        //             node::NodeName::new(),
        //             level_map::CoordPos::default(),
        //             transform::position::Position::default(), 
        //             custom_mesh::Material::from_str("res://select_box.material")
        //         )
        //     ]
        // );

        let schedule = Schedule::builder()
            .add_system(input::create_system())
            .add_system(camera::create_system())
            .add_system(level_map::create_system())
            .add_system(selection_box::create_system())
            .flush()
            //systems which add nodes should go first
            .add_thread_local(custom_mesh::create_system_local())
            //systems that work on nodes follow
            .add_thread_local_fn(selection_box::create_thread_local_fn())
            .add_thread_local(transform::position::create_system_local())
            .add_thread_local(transform::rotation::create_system_local())
            .build();

        self.schedule = Some(schedule);
    }

    #[export]
    fn _process(&mut self, _owner: Node, delta: f64) {
        unsafe { DELTA_TIME = delta };

        let world = self.world.as_mut().unwrap();
        
        let schedule = self.schedule.as_mut().unwrap();
        schedule.execute(world);
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