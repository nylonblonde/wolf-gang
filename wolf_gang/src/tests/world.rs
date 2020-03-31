use std::collections::HashMap;
use legion::prelude::*;
use crate::level_map;
use std::sync::{Arc, Mutex};

type AABB = crate::geometry::aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;

#[test]
fn detect_map_chunk_change() {
    let universe = Universe::new();
    let mut world = universe.create_world();

    let mut resources = Resources::default();
    resources.insert(level_map::Map::default());

    let insert_count = Arc::new(Mutex::new(0u32));
    let insert_counter = insert_count.clone();

    // // Same results whether I create the chunks beforehand or not
    // let chunk_pt1 = Point::new(0,0,0);
    // let chunk_pt2 = Point::new(-1,0,0);

    // world.insert((chunk_pt1,), vec![
    //     (level_map::MapChunkData::new(AABB::new(Point::new(5,5,5), Point::new(10,10,10))),)
    // ]);

    // world.insert((chunk_pt2,), vec![
    //     (level_map::MapChunkData::new(AABB::new(Point::new(-5,5,5), Point::new(10,10,10))),)
    // ]);

    world.insert((0u32,), vec![
        (Vec::<u32>::new(),)
    ]);

    world.insert((1u32,), vec![
        (Vec::<u32>::new(),)
    ]);

    let insert_values_fn = Box::new(move |world: &mut legion::world::World, _: &mut Resources|{
        let mut count = insert_counter.lock().unwrap();
        
        let mut entities = Vec::<Entity>::new();

        for i in 0..2 {

            println!("Looking for entity tagged {}", i);

            let query = <Tagged<u32>>::query()
                    // .filter(tag_value(&i))
                    ;

            for (entity, tag) in query.iter_entities(world) {
                println!("Looking at entity tagged as {}", tag);
                entities.push(entity);
            }
        }

        let mut to_add: HashMap<Entity, Vec<u32>> = HashMap::new();

        for entity in entities {
            let mut component = world.get_component_mut::<Vec<u32>>(entity).unwrap();
            println!("Vec has a length of {}", component.len());
            component.push(*count);

            to_add.insert(entity, component.clone());
        }
        
        for (entity, component) in to_add {
            world.add_component(entity, component).unwrap();
        }

        // let point = Point::new(0,0,*count as i32);

        // let map = resources.get::<level_map::Map>().unwrap();
        // let tile_data = level_map::TileData::new(Point::zeros());
        // let aabb = AABB::new(point, Point::new(2,1,1));

        //This function checks if the map chunks which would fit the aabb exist, creates them if not, and then inserts
        // the tile_data
        // map.insert(world, tile_data, aabb);
        // println!("Inserting at {:?}", point);

        *count += 1;
    });

    let detect_change_system = SystemBuilder::new("detect_change_system")
        .with_query(<Read<Vec<u32>>>::query()
            //Test passes if filter is commented out
            .filter(changed::<Vec<u32>>())
        )
        .build(move |_, world, _, query| {
            assert_eq!(query.iter(world).count() as u32, 2);
        });

    let mut schedule = Schedule::builder()
        .add_thread_local_fn(insert_values_fn)
        .add_system(detect_change_system)
        .build();

    // loop in place of the full game loop
    for _ in 0..9 {
        schedule.execute(&mut world, &mut resources);
    }
}