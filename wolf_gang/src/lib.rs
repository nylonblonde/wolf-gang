#![feature(cmp_min_max_by)]
#![allow(dead_code)]

use gdnative::*;

#[macro_use]
extern crate lazy_static;

use legion::prelude::*;

use std::sync::Mutex;

mod collections;
mod geometry;
mod systems;
mod node;
mod history;

use systems::{camera, input, level_map, custom_mesh, selection_box, smoothing, transform, udp};

mod nodes;

#[cfg(test)]
mod tests;

static mut OWNER_NODE: Option<Node> = None;

lazy_static! {
    static ref GAME_UNIVERSE: Mutex<GameUniverse> = Mutex::new( 

        {
            let universe = Universe::new();
            let world = universe.create_world();

            GameUniverse{
                universe,
                world,
                resources: Resources::default(),
            }
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
    schedule: Option<Schedule>,
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl WolfGang {
    
    /// The "constructor" of the class.
    fn _init(owner: Node) -> Self {

        unsafe { OWNER_NODE = Some(owner); }

        let wolf_gang = WolfGang {
            schedule: None,
        };

        wolf_gang
    }
    
    // In order to make a method known to Godot, the #[export] attribute has to be used.
    // In Godot script-classes do not actually inherit the parent class.
    // Instead they are"attached" to the parent object, called the "owner".
    // The owner is passed to every single exposed method.
    #[export]
    fn _ready(&mut self, _owner: Node) {

        godot_print!("hello, world.");

        let mut game = GAME_UNIVERSE.lock().unwrap();

        let resources = &mut game.resources;

        resources.insert(Time{
            delta: 0.
        });

        resources.insert(udp::ClientSocket::new("127.0.0.1:12345"));

        resources.insert(udp::ServerSocket::new("127.0.0.1:12346"));

        resources.insert(level_map::Map::default());    

        resources.insert(history::CurrentHistoricalStep::default());

        resources.insert(level_map::document::Document::default());

        let mut world = &mut game.world;
        input::initialize_input_config(&mut world, input::CONFIG_PATH);

        let camera = camera::initialize_camera(&mut world);

        // world.insert(
        //     (),
        //     (0..1).map(|_| (level_map::MapChunkData::new(), custom_mesh::MeshData::new(),))
        // );

        // world.insert(
        //     (),
        //     (0..1).map(|_| (level_map::Map::new(),))
        // );

        // let camera = "".to_string();
        selection_box::initialize_selection_box(&mut world, camera);

        // let test_node = node::NodeName("Cone".to_string());
        //     world.insert(
        //         (test_node.clone(),),
        //         vec![
        //             (
        //                 transform::rotation::Rotation::default(),
        //                 transform::rotation::Direction::default(),
        //                 transform::position::Position::default(),
        //                 camera::FocalPoint::default(),
        //                 camera::FocalAngle(-45.0f32.to_radians(),45.0f32.to_radians(),0.),
        //                 camera::Zoom(5.0)
        //             )
        //         ]
        //     );

        // let test_system = Box::new(|world: &mut legion::world::World|{
        //     let node_name = node::NodeName("Cone".to_string());

        //     let query = <(Write<camera::FocalAngle>,)>::query()
        //         .filter(tag_value(&node_name));

        //     unsafe {
        //         for (mut focal_angle,) in query.iter_unchecked(world) {
        //             focal_angle.1 += 1.0f32.to_radians();
        //         }
        //     }
        // });

        let schedule = Schedule::builder()
            .add_thread_local_fn(input::create_thread_local_fn())
            .add_system(smoothing::create_system())
            .add_system(camera::create_movement_system())
            .add_system(camera::create_rotation_system())
            .add_system(level_map::mesh::create_add_material_system())
            .add_system(selection_box::create_system())
            .add_system(selection_box::create_coord_to_pos_system())
            .add_system(custom_mesh::create_tag_system())
            .flush()
            //systems which add nodes should go first
            .add_thread_local(custom_mesh::create_draw_system_local())
            //systems that work on nodes follow
            
            .add_thread_local_fn(selection_box::create_orthogonal_dir_thread_local_fn())
            .add_thread_local_fn(selection_box::create_movement_thread_local_fn())
            .add_thread_local_fn(selection_box::create_expansion_thread_local_fn())
            .add_thread_local_fn(selection_box::create_tile_tool_thread_local_fn())
            .add_thread_local_fn(level_map::history::create_undo_redo_input_system())
            .add_thread_local_fn(level_map::mesh::create_drawing_thread_local_fn())
            .add_thread_local_fn(camera::create_focal_point_thread_local_fn())
            .add_thread_local_fn(camera::create_camera_angle_thread_local_fn())
            .add_thread_local_fn(camera::create_follow_selection_box_thread_local_fn())
            // .add_thread_local_fn(test_system)
            .add_thread_local(transform::position::create_system_local())
            .add_thread_local(transform::rotation::create_system_local())
            .build();

        self.schedule = Some(schedule);
    }

    #[export]
    fn _process(&mut self, _owner: Node, delta: f64) {

        let mut game = GAME_UNIVERSE.lock().unwrap();

        let game = &mut *game;

        let mut resources = &mut game.resources;

        resources.insert(Time{
            delta: delta as f32
        });

        let mut world = &mut game.world;

        let schedule = self.schedule.as_mut().unwrap();
        schedule.execute(&mut world, &mut resources);
    }
}

// Function that registers all exposed classes to Godot
fn init(handle: gdnative::init::InitHandle) {
    handle.add_class::<WolfGang>();
    handle.add_class::<nodes::edit_menu::EditMenu>();
    handle.add_class::<nodes::file_menu::FileMenu>();
    handle.add_class::<nodes::file_dialog::SaveLoadDialog>();
}

// macros that create the entry-points of the dynamic library.
godot_gdnative_init!();
godot_nativescript_init!(init);
godot_gdnative_terminate!();