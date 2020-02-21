use legion::prelude::*;
use crate::transform::position::Position;
use crate::node::NodeName;

use std::sync::{Arc, Mutex};

#[test]
fn test_add_tag() {

    let universe = Universe::new();
    let mut world = universe.create_world();

    let mut entity = Arc::new(Mutex::new(
        world.insert((NodeName(Some("Something Else".to_string())),), vec![(Position::default(),)])[0]
    ));


    let system = SystemBuilder::new("one")
        .with_query(<(Read<Position>,)>::query())
        .build_thread_local(move |cmd, world, _, query| {
            for (entity, (pos,)) in query.iter_entities(&mut *world) {
                cmd.add_tag(entity, NodeName(Some("Test".to_string())));
            }
        }
    );

    let mut schedule = Schedule::builder().add_thread_local(system).build();

    schedule.execute(&mut world, &mut Resources::default());

    let entity = entity.lock().unwrap();

    // world.add_tag(entity, NodeName(Some("Test".to_string())));
    
    assert_eq!(world.get_tag::<NodeName>(*entity).unwrap().0.as_ref().unwrap(), &"Test".to_string());

}

#[test]
fn test_read_resource() {
    let universe = Universe::new();
    let mut worls = universe.create_world();

    let mut resources = Resources::default();
    resources.insert

    world.insert()
}