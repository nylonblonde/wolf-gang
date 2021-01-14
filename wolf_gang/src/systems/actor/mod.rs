use legion::*;
use legion::{
    world::{Allocate},
    serialize::{CustomEntitySerializer}
};
use gdnative::prelude::*;
use gdnative::api::{
    File
};

use serde::{Serialize, Deserialize, de::DeserializeSeed};

use bincode::Options;

type Point = nalgebra::Vector3<i32>;

use crate::{
    node, 
    node::{
        NodeParent,
        NodeRef,
    },
    systems::{
        history::{History, StepType},
        level_map::{CoordPos, TILE_DIMENSIONS, map_coords_to_world},
        transform::{
            position::Position,
            rotation::Rotation,
        },
        networking::ClientID,
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
            registry.register::<CoordPos>("coord_pos".to_string());
            registry.register::<Rotation>("rotation".to_string());
            
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
            merger.register_copy::<CoordPos>();
            merger.register_copy::<Rotation>();

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
    ActorRemoval(u128)
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

pub fn create_move_to_coord_system() -> impl systems::Runnable {
    SystemBuilder::new("actor_move_to_coord_system")
        .with_query(<(Entity, Read<Bounds>, Read<Rotation>, Read<CoordPos>)>::query()
            .filter(component::<ActorID>() & maybe_changed::<CoordPos>() | maybe_changed::<Rotation>()))
        .build(move |commands, world, _, query| {
            query.iter(world)
                .map(|(entity, bounds, rotation, coord_pos)| (*entity, *bounds, *rotation, *coord_pos))
                .collect::<Vec<(Entity, Bounds, Rotation, CoordPos)>>()
                .into_iter()
                .for_each(|(entity, bounds, rotation, coord_pos)| {
                    let mut aabb = bounds.get_scaled_and_rotated_aabb(rotation.value);

                    aabb.center = coord_pos.value;

                    commands.exec_mut(move |world, _| {
                        position_actor_helper(world, entity, aabb);
                    });
                })
        })
}

pub fn position_actor_helper(world: &mut World, actor_entity: Entity, aabb: AABB) {
    if let Some(mut entry) = world.entry(actor_entity) {

        let min = map_coords_to_world(aabb.get_min());

        let bounds = map_coords_to_world(aabb.dimensions);
        
        let position = Position {
            value: nalgebra::Vector3::new(min.x, min.y, min.z) + nalgebra::Vector3::new(bounds.x/2., 0., bounds.z/2.)
        };

        entry.add_component(position);
        
    }
}

pub fn serialize_actors_in_world(world: &mut World) -> Result<Vec<u8>, bincode::Error> {
    let mut actor_world = World::default();
    MERGER.with(|m| {
        let mut merger = m.borrow_mut();
        actor_world.clone_from(world, &component::<ActorID>(), &mut *merger);

        REGISTRY.with(|r| {
            let registry = r.borrow();

            CANON.with(|c| {
                let canon = c.borrow();

                bincode::serialize(&actor_world.as_serializable(component::<ActorID>(), & *registry, & *canon))
            })
        })
    })
}

pub fn serialize_single_actor_in_world(world: &mut World, entity: Entity) -> Result<Vec<u8>, bincode::Error> {
    let mut actor_world = World::default();
    MERGER.with(|m| {
        let mut merger = m.borrow_mut();
        actor_world.clone_from_single(world, entity, &mut *merger);

        REGISTRY.with(|r| {
            let registry = r.borrow();

            CANON.with(|c| {
                let canon = c.borrow();

                bincode::serialize(&actor_world.as_serializable(component::<ActorID>(), & *registry, & *canon))
            })
        })
    })
}

pub fn change(world: &mut World, change: &ActorChange, store_history: Option<u32>) {
    match change {

        ActorChange::ActorInsertion{serialized} => {
        REGISTRY.with(|r| {
            let registry = r.borrow();

            CANON.with(move |c| {
                let canon = c.borrow();
                
                let mut deserialized = bincode::de::Deserializer::from_slice(
                    &serialized[..], 
                    bincode::config::DefaultOptions::new()
                        .with_fixint_encoding()
                        .allow_trailing_bytes()
                );

                let actor_world: World = registry.as_deserialize(& *canon).deserialize(&mut deserialized).unwrap();

                let mut query = <(Entity, Read<ActorID>)>::query();
                query.iter(&actor_world)
                    .map(|(actor_entity, actor_id)| (*actor_entity, *actor_id))
                    .collect::<Vec<(Entity, ActorID)>>()
                    .into_iter()
                    .for_each(|(actor_entity, actor_id)| {

                        let world_actors = query.iter(world)
                            .map(|(actor_entity, actor_id)| (*actor_entity, *actor_id))
                            .collect::<Vec<(Entity, ActorID)>>();

                        if world_actors.is_empty() || !world_actors.into_iter().any(|(_,id)| id.val() == actor_id.val()) {
                            
                            if let Some(store_history) = store_history {
                                let mut history_query = <(Write<History>, Read<ClientID>)>::query();

                                history_query.iter_mut(world).filter(|(_, id)| id.val() == store_history).for_each(|(history, _)| {
                                    history.add_step(
                                        StepType::ActorChange(
                                            (ActorChange::ActorRemoval(actor_id.val()), change.clone())
                                        )
                                    );
                                });
                            }

                            MERGER.with(|m| {
                                let mut merger = m.borrow_mut();
                                world.clone_from_single(&actor_world, actor_entity, &mut *merger);

                            });
                        }
                });
            });
        });
        },
        ActorChange::ActorRemoval(actor_id) => {

            let mut query = <(Entity, Read<ActorID>, Read<NodeRef>)>::query();
            if let Some((entity, node)) = query.iter(world)
                .find(|(_, id, _)| id.val() == *actor_id)
                .map(|(entity, _, node_ref)| (*entity, node_ref.val())) {

                    if let Some(store_history) = store_history {
                        let mut history_query = <(Write<History>, Read<ClientID>)>::query();
                        if let Ok(serialized) = serialize_single_actor_in_world(world, entity) {
                            if let Some((history, _)) = history_query.iter_mut(world).find(|(_, id)| id.val() == store_history) {
                                history.add_step(
                                    StepType::ActorChange(
                                        (ActorChange::ActorInsertion{
                                            serialized: serialized.to_vec()
                                        }, change.clone())
                                    )
                                )
                            }
                        }
                    }
                    node::free(world, node);
                }
        }
    }
}

pub fn free_all(world: &mut World) {
    let mut actor_query = <Read<NodeRef>>::query().filter(component::<ActorID>());

    actor_query.iter(world)
        .map(|node_ref| node_ref.val())
        .collect::<Vec<Ref<Node>>>()
        .into_iter()
        .for_each(|node| {
            node::free(world, node);
        })
}

pub fn select_actors_from_range(world: &mut World, range: AABB) -> Vec<Entity> {
    let mut actor_query = <(Entity, Read<Bounds>, Read<Rotation>, Read<CoordPos>)>::query().filter(component::<ActorID>());

    actor_query.iter(world)
        .filter(|(_, bounds, rotation, coord_pos)| {
            let mut aabb = bounds.get_scaled_and_rotated_aabb(rotation.value);
            aabb.center = coord_pos.value;

            range.intersects_bounds(aabb)
        })
        .map(|(entity, _, _, _)| *entity)
        .collect::<Vec<Entity>>()
}
