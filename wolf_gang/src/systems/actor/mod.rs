use legion::*;
use legion::{
    world::{Allocate},
    serialize::{CustomEntitySerializer}
};
use gdnative::prelude::*;
use gdnative::api::{
    File
};

use serde::{Serialize, Deserialize};

type Point = nalgebra::Vector3<i32>;

use crate::{
    node, 
    node::{
        NodeParent,
        NodeRef,
    },
    systems::{
        level_map::{TILE_DIMENSIONS, map_coords_to_world},
        transform::{
            position::Position,
        }
    },
};

type AABB = crate::geometry::aabb::AABB<i32>;

use std::{
    collections::HashMap,
    cell::RefCell,
};

thread_local! {
    pub static REGISTRY: RefCell<Registry<String>> = RefCell::new(
        {
            let mut registry = Registry::default();

            registry.register::<Actor>("actor".to_string());
            registry.register::<ActorID>("actor_id".to_string());
            registry.register::<Bounds>("bounds".to_string());
            registry.register::<PlayableCharacter>("playable_character".to_string());
            registry.register::<ActorSceneKey>("actor_scene_key".to_string());
            registry.register::<Health>("health".to_string());
            
            registry
        }
    );

    pub static MERGER: RefCell<legion::world::Duplicate> = RefCell::new(
        {
            let mut merger = legion::world::Duplicate::default();

            merger.register_clone::<Actor>();
            merger.register_copy::<ActorID>();
            merger.register_copy::<Bounds>();
            merger.register_clone::<PlayableCharacter>();
            merger.register_clone::<ActorSceneKey>();
            merger.register_copy::<Health>();

            merger
        }
    );

    pub static ACTOR_SCENE_MAP: RefCell<HashMap<String, String>> = RefCell::new(
        {
            let file = File::new();
            file.open("res://config/actor_paths.ron", File::READ).expect("Failed to open res://config/actor_paths.ron");
            let file_string = file.get_as_text().to_string();
            ron::de::from_str::<HashMap<String, String>>(file_string.as_str()).expect("Failed to deserialize the config/actor_paths.ron file")
        }
    );

    pub static CANON: RefCell<legion::serialize::Canon> = RefCell::new(legion::serialize::Canon::default());
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor(pub String);

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Bounds(pub nalgebra::Vector3::<f32>);

impl Bounds {
    pub fn get_scaled_and_rotated_aabb(&self, rotation: nalgebra::Rotation3<f32>) -> AABB {
        let dimensions = self.0;

        let scaled = Point::new(
            (dimensions.x/ TILE_DIMENSIONS.x) as i32,
            (dimensions.y/ TILE_DIMENSIONS.y) as i32,
            (dimensions.z/ TILE_DIMENSIONS.z) as i32,
        );

        let aabb = AABB::new(Point::zeros(), scaled);

        aabb.rotate(rotation)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayableCharacter(pub Option<Character>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSceneKey(pub String);

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Health {
    max_health: u32,
    health: u32
}

#[derive(Default)]
pub struct MyEntitySerializerInner {
    allocator: Allocate,
}

#[derive(Default)]
pub struct MyEntitySerializer{
    inner: parking_lot::RwLock<MyEntitySerializerInner>
}

impl CustomEntitySerializer for MyEntitySerializer {
    
    type SerializedID = String;
    /// Doesn't get used but has to be implemented
    fn to_serialized(&self, _: Entity) -> Self::SerializedID { 
        String::default()
    }

    fn from_serialized(&self, _: Self::SerializedID) -> Entity { 
        self.inner.write().allocator.next().unwrap()
    }

}

impl Health {

    pub fn new(max_health: u32) -> Self {
        Self{
            max_health,
            health: max_health
        }
    }

    pub fn get_max_health(&self) -> u32 {
        self.max_health
    }

    /// Used to add or subtract to player health
    pub fn add_health(&mut self, addition: i32) {
        let new_value = self.health as i32 + addition;
        self.health = std::cmp::min(std::cmp::max(new_value, 0), self.max_health as i32) as u32;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActorChange {
    ActorInsertion {
        serialized: Vec<u8>
    },
    ActorRemoval(Entity)
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct ActorID(u128);

impl ActorID {

    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().as_u128())
    }

    pub fn val(&self) -> u128 {
        self.0
    }
}

pub fn create_initialize_actor_scene_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {

    let mut query = <(Entity, Read<ActorSceneKey>)>::query().filter(!component::<NodeRef>());

    Box::new(move |world, _| {
        ACTOR_SCENE_MAP.with(|a| {
            let actor_scene_map = a.borrow();
    
            query.iter(world).map(|(entity, actor_key)| (*entity, actor_key.clone()))
                .collect::<Vec<(Entity, ActorSceneKey)>>()
                .into_iter()
                .for_each(|(entity, actor_key)| {

                    if let Some(mut entry) = world.entry(entity) { 

                        let parent = entry.get_component::<NodeParent>().map(|node_parent| node_parent.val()).unwrap_or(
                            unsafe { crate::OWNER_NODE.unwrap() }
                        );
                        
                        let actor_node = match actor_scene_map.get(&actor_key.0) {
                            Some(path) => node::init_scene(unsafe { &parent.assume_safe() }, path),
                            None => todo!("Proper error handling for nonexistent data, maybe load a default actor")
                        };

                        entry.add_component(NodeRef::new(actor_node)); 
                    } 
                });  
        });
    }) 
}

pub fn position_actor_helper(world: &mut World, actor_entity: Entity, aabb: AABB) {
    if let Some(mut entry) = world.entry(actor_entity) {

        let min = map_coords_to_world(aabb.get_min());

        let bounds = map_coords_to_world(aabb.dimensions);
        let bounds = Vector3::new(bounds.x, bounds.y, bounds.z);
        
        let position = Position {
            value: Vector3::new(min.x, min.y, min.z) + Vector3::new(bounds.x/2., 0., bounds.z/2.)
        };

        entry.add_component(position);
        
    }
}
