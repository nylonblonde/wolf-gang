#![feature(cmp_min_max_by)]
#![feature(inner_deref)]
#![feature(ip)]

#![allow(dead_code)]

use gdnative::prelude::*;

#[macro_use]
extern crate lazy_static;

use legion::*;

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc, Mutex, RwLock
    },
};

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

thread_local! {
   pub static STATE_MACHINE: RefCell<game_state::StateMachine> = RefCell::new(
        game_state::StateMachine{
            states: Vec::new()
        }
    );
}

pub struct GameUniverse {
    pub world: legion::world::World,
}


pub struct Time {
    delta: f32
}

/// The WolfGang "class"
#[derive(NativeClass)]
#[inherit(Node)]
#[user_data(user_data::LocalCellData<WolfGang>)]
pub struct WolfGang {
    resources: Rc<RefCell<Resources>>,
    world: Arc<RwLock<World>>,
    // schedule: Option<Schedule>,
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl WolfGang {

    pub fn get_world() -> Option<Arc<RwLock<World>>> {
        let owner = unsafe { OWNER_NODE.unwrap().assume_safe() };
        let instance = owner.cast_instance::<WolfGang>();

        match instance {
            Some(instance) =>  
                instance.map(|inst, _| 
                    Some(Arc::clone(&inst.world))
                ).unwrap_or_else(|_| None),
            _ => None
        }
    }

    pub fn get_resources() -> Option<Rc<RefCell<Resources>>> {
        let owner = unsafe { OWNER_NODE.unwrap().assume_safe() };
        let instance = owner.cast_instance::<WolfGang>();

        match instance {
            Some(instance) =>  
                instance.map(|inst, _| 
                    Some(Rc::clone(&inst.resources))
                ).unwrap_or_else(|_| None),
            _ => None
        }
    }

    /// The "constructor" of the class.
    fn new(owner: &Node) -> Self {

        unsafe { OWNER_NODE = Some(owner.assume_shared()); }

        WolfGang{
            resources: Rc::new(RefCell::new(Resources::default())),
            world: Arc::new(RwLock::new(World::default()))
        }
    }
    
    // In order to make a method known to Godot, the #[export] attribute has to be used.
    // In Godot script-classes do not actually inherit the parent class.
    // Instead they are"attached" to the parent object, called the "owner".
    // The owner is passed to every single exposed method.
    #[export]
    fn _ready(&mut self, _owner: &Node) {

        godot_print!("hello, world.");

        let world = &mut *self.world.write().unwrap();
        let resources = &mut *self.resources.borrow_mut();
        
        resources.insert(Time{
            delta: 0.
        });

        systems::input::initialize_input_config(world, systems::input::CONFIG_PATH);

        STATE_MACHINE.with(|s| {
            let mut state_machine = s.borrow_mut();

            state_machine.add_state(
                networking::Networking::new("Networking", 
                    Schedule::builder()
                        .add_system(systems::networking::create_server_system())
                        .add_system(systems::networking::create_client_system())
                        .add_thread_local_fn(systems::networking::create_new_connection_thread_local_fn())
                        .add_thread_local_fn(systems::networking::create_disconnection_thread_local_fn())
                        .build(),
                    true
                ),
                world, resources
            );
            
            state_machine.add_state(
                editor::Editor::new("MapEditor", 
                    Schedule::builder()
                        .add_system(systems::input::create_input_system())                             
                        .flush() //flush to avoid accidental double inputs

                        .add_system(systems::smoothing::create_system())
                        .add_system(systems::camera::create_movement_system())
                        .add_system(systems::camera::create_rotation_system())
                        .add_system(systems::selection_box::create_system())
                        .add_system(systems::selection_box::create_move_to_system())
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

                        .build(),
                    true
                ), 
                world, resources
            );

        });
    }

    #[export]
    fn _process(&mut self, _owner: &Node, delta: f64) {

        let mut world = &mut self.world.write().unwrap();
        let mut resources = &mut *self.resources.borrow_mut();

        resources.insert(Time{
            delta: delta as f32
        });

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