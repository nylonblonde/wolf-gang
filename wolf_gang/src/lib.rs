#![feature(cmp_min_max_by)]

use gdnative::*;

use legion::prelude::*;
use lazy_static::*;

use std::collections::HashMap;

mod collections;
mod geometry;
mod systems;
mod node;

use systems::{camera, input, level_map, custom_mesh, selection_box, smoothing, transform};

#[cfg(test)]
mod tests;

static mut OWNER_NODE: Option<Node> = None;

pub struct Time {
    delta: f32
}

/// The WolfGang "class"
#[derive(NativeClass)]
#[inherit(Node)]
#[user_data(user_data::LocalCellData<WolfGang>)]
pub struct WolfGang {
    universe: Option<Universe>,
    world: Option<legion::world::World>,
    schedule: Option<Schedule>,
    resources: Option<Resources>,
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl WolfGang {
    
    /// The "constructor" of the class.
    fn _init(owner: Node) -> Self {

        unsafe { OWNER_NODE = Some(owner); }

        WolfGang {
            universe: None,
            world: None,
            schedule: None,
            resources: None,
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

        self.resources = Some(Resources::default());

        let resources = self.resources.as_mut().unwrap();

        resources.insert(Time{
            delta: 0.
        });

        let mut world = self.world.as_mut().unwrap();
        input::initialize_input_config(world, input::CONFIG_PATH);

        let camera = camera::initialize_camera(&mut world);

        // world.insert(
        //     (),
        //     (0..1).map(|_| (level_map::MapChunkData::new(), custom_mesh::MeshData::new(),))
        // );

        world.insert(
            (),
            (0..1).map(|_| (level_map::Map::new(),))
        );

        // let camera = "".to_string();
        selection_box::initialize_selection_box(world, camera);

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
            .add_system(level_map::create_system())
            .add_system(selection_box::create_system())
            .add_system(selection_box::create_coord_to_pos_system())
            .flush()
            //systems which add nodes should go first
            .add_thread_local(custom_mesh::create_system_local())
            //systems that work on nodes follow
            .add_thread_local_fn(selection_box::create_orthogonal_dir_thread_local_fn())
            .add_thread_local_fn(selection_box::create_movement_thread_local_fn())
            .add_thread_local_fn(selection_box::create_expansion_thread_local_fn())
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

        let world = self.world.as_mut().unwrap();

        let resources = self.resources.as_mut().unwrap();

        resources.insert(Time{
            delta: delta as f32
        });
        
        let schedule = self.schedule.as_mut().unwrap();
        schedule.execute(world, resources);
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