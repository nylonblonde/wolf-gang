#![feature(cmp_min_max_by)]
#![feature(inner_deref)]
#![feature(ip)]

#![allow(dead_code)]

use gdnative::prelude::*;

#[macro_use]
extern crate lazy_static;

use legion::*;

use std::sync:: Mutex;
use std::cell::RefCell;

mod collections;
mod geometry;
mod systems;
mod node;
mod editor;
mod game_state;
mod networking;

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

        systems::input::initialize_input_config(world, systems::input::CONFIG_PATH);

        STATE_MACHINE.with(|s| {
            let mut state_machine = s.borrow_mut();
            
            state_machine.add_state(
                editor::Editor::new("MapEditor", 
                    Schedule::builder()
                        .add_system(systems::input::create_input_system())                             
                        .flush() //flush to avoid accidental double inputs

                        .add_system(systems::smoothing::create_system())
                        .add_system(systems::camera::create_movement_system())
                        .add_system(systems::camera::create_rotation_system())
                        .add_system(systems::selection_box::create_system())
                        .add_system(systems::selection_box::create_coord_to_pos_system())
                        
                        .add_system(systems::selection_box::create_tile_tool_system())

                        .add_system(systems::custom_mesh::create_tag_system())

                        .add_system(systems::camera::create_camera_angle_system())
                        .add_system(systems::camera::create_focal_point_system())
                        .add_system(systems::camera::create_follow_selection_box_system())

                        .add_system(systems::selection_box::create_orthogonal_dir_system())
                        .add_system(systems::selection_box::create_movement_system()) 
                        .add_system(systems::selection_box::create_expansion_system())

                        .add_system(systems::level_map::mesh::create_add_components_system())
                        .add_system(systems::level_map::mesh::create_drawing_system())
                        .flush() //need to flush before drawing custom meshes
                        .add_system(systems::custom_mesh::create_draw_system())

                        .add_system(systems::transform::rotation::create_system())
                        .add_system(systems::transform::position::create_system())
                        
                        .add_system(systems::level_map::create_map_input_system())
                        .add_system(systems::history::create_history_input_system())

                        .add_system(systems::networking::create_new_connection_system())
                        .add_system(systems::networking::create_message_pooling_system())
                        .build(),
                    true
                ),
                world, resources);

                state_machine.add_state(
                networking::Networking::new("Networking", 
                    Schedule::builder()
                        .add_system(systems::networking::create_message_pooling_system())
                        .build(),
                    true),
                world, resources
            );
        });
    }

    #[export]
    fn _process(&mut self, _owner: &Node, delta: f64) {

        let mut game_lock = GAME_UNIVERSE.lock().unwrap();

        let game = &mut *game_lock;

        let mut resources = &mut game.resources;

        resources.insert(Time{
            delta: delta as f32
        });

        let mut world = &mut game.world;

        STATE_MACHINE.with(|s| {
            for state in &s.borrow().states {
                let state = state.as_ref();

                let game_state: &GameState = state.as_ref();
                if game_state.is_active() {
                    game_state.schedule.borrow_mut().execute(&mut world, &mut resources);
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
    handle.add_class::<nodes::connect_menu::ConnectMenu>();
    handle.add_class::<nodes::connet_dialog::ConnectDialog>();
}

godot_init!(init);