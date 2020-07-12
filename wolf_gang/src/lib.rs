#![feature(cmp_min_max_by)]
#![feature(inner_deref)]

#![allow(dead_code)]

use gdnative::prelude::*;

#[macro_use]
extern crate lazy_static;

use legion::prelude::*;

use std::sync:: Mutex;
use std::cell::RefCell;

mod collections;
mod geometry;
mod systems;
mod node;
mod history;
mod editor;
mod game_state;

use game_state::{GameState, NewState};

mod nodes;

#[cfg(test)]
mod tests;

static mut OWNER_NODE: Option<Ref<Node>> = None;

lazy_static! {
    static ref GAME_UNIVERSE: Mutex<GameUniverse> = Mutex::new( 

        {
            let universe = Universe::new();
            let world = universe.create_world();

            GameUniverse {
                universe,
                world,
                resources: Resources::default(),
            }
        }
    );
}

thread_local! {

   pub static STATE_MACHINE: RefCell<game_state::StateMachine> = RefCell::new(
        game_state::StateMachine{
            states: Vec::new()
        }
    );
}

pub struct GameUniverse {
    pub universe: Universe,
    pub world: legion::world::World,
    pub resources: Resources,
}


pub struct Time {
    delta: f32
}

/// The WolfGang "class"
#[derive(NativeClass)]
#[inherit(Node)]
#[user_data(user_data::LocalCellData<WolfGang>)]
pub struct WolfGang {
    // schedule: Option<Schedule>,
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl WolfGang {
    
    /// The "constructor" of the class.
    fn new(owner: &Node) -> Self {

        unsafe { OWNER_NODE = Some(owner.assume_shared()); }

        WolfGang{}
    }
    
    // In order to make a method known to Godot, the #[export] attribute has to be used.
    // In Godot script-classes do not actually inherit the parent class.
    // Instead they are"attached" to the parent object, called the "owner".
    // The owner is passed to every single exposed method.
    #[export]
    fn _ready(&mut self, _owner: &Node) {

        godot_print!("hello, world.");
        
        let mut game = GAME_UNIVERSE.lock().unwrap();
        let game = &mut *game;

        let resources = &mut game.resources;
        let world = &mut game.world;

        resources.insert(Time{
            delta: 0.
        });
        resources.insert(systems::udp::ClientSocket::new("127.0.0.1:12345"));
        resources.insert(systems::udp::ServerSocket::new("127.0.0.1:12346"));
        systems::input::initialize_input_config(world, systems::input::CONFIG_PATH);

        STATE_MACHINE.with(|s| {
            let mut state_machine = s.borrow_mut();
            state_machine.add_state(
                editor::Editor::new("MapEditor", 
                    Schedule::builder()
                        .add_thread_local_fn(systems::input::create_thread_local_fn())
                        .add_system(systems::smoothing::create_system())
                        .add_system(systems::camera::create_movement_system())
                        .add_system(systems::camera::create_rotation_system())
                        .add_system(systems::selection_box::create_system())
                        .add_system(systems::selection_box::create_coord_to_pos_system())
                        .add_system(systems::custom_mesh::create_tag_system())
                        .flush()
                        //systems which add nodes should go first
                        .add_thread_local(systems::custom_mesh::create_draw_system_local())
                        //systems that work on nodes follow
                        
                        .add_thread_local_fn(systems::selection_box::create_orthogonal_dir_thread_local_fn())
                        .add_thread_local_fn(systems::selection_box::create_movement_thread_local_fn())
                        .add_thread_local_fn(systems::selection_box::create_expansion_thread_local_fn())
                        .add_thread_local_fn(systems::selection_box::create_tile_tool_thread_local_fn())
                        .add_thread_local_fn(systems::level_map::mesh::create_add_components_system())
                        .add_thread_local_fn(systems::level_map::history::create_undo_redo_input_system())
                        .add_thread_local_fn(systems::level_map::mesh::create_drawing_thread_local_fn())
                        .add_thread_local_fn(systems::camera::create_focal_point_thread_local_fn())
                        .add_thread_local_fn(systems::camera::create_camera_angle_thread_local_fn())
                        .add_thread_local_fn(systems::camera::create_follow_selection_box_thread_local_fn())
                        // // .add_thread_local_fn(test_system)
                        .add_thread_local(systems::transform::rotation::create_system_local())
                        .add_thread_local(systems::transform::position::create_system_local())
                        .build(),
                    true
                ),
                world, resources);
        });
    }

    #[export]
    fn _process(&mut self, _owner: &Node, delta: f64) {

        let mut game = GAME_UNIVERSE.lock().unwrap();

        let game = &mut *game;

        let mut resources = &mut game.resources;

        resources.insert(Time{
            delta: delta as f32
        });

        let mut world = &mut game.world;

        STATE_MACHINE.with(|s| {
            for state in &mut s.borrow_mut().states {
                let state = state.as_mut();

                let game_state: &mut GameState = state.as_mut();
                if game_state.is_active() {
                    game_state.schedule.execute(&mut world, &mut resources);
                }
            }
        });
    }
}

// Function that registers all exposed classes to Godot
fn init(handle: InitHandle) {
    handle.add_class::<WolfGang>();
    handle.add_class::<nodes::edit_menu::EditMenu>();
    handle.add_class::<nodes::file_menu::FileMenu>();
    handle.add_class::<nodes::file_dialog::SaveLoadDialog>();
    handle.add_class::<nodes::file_confirmation::FileConfirmation>();
}

godot_init!(init);

// // macros that create the entry-points of the dynamic library.
// godot_gdnative_init!();
// godot_nativescript_init!(init);
// godot_gdnative_terminate!();