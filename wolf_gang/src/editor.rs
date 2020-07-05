use legion::prelude::*;
use crate::{
    game_state::{NewState, GameState, GameStateTraits},
    history,
    systems::{
        camera,
        level_map,
        selection_box,
    },
};

use std::{
    cell::RefCell,
    rc::Rc
};


pub struct Editor {
    game_state: GameState,
    initialize: Box<dyn FnMut(&mut World, &mut Resources)>,
    free: Box<dyn FnMut(&mut World, &mut Resources)>,
    camera: Rc<RefCell<String>>,
    // map: level_map::Map,
}

impl GameStateTraits for Editor {
    fn initialize_func(&mut self) -> &mut Box<dyn FnMut(&mut World, &mut Resources)> {
        &mut self.initialize
    }
    fn free_func(&mut self) -> &mut Box<dyn FnMut(&mut World, &mut Resources)> {
        &mut self.free
    }
}

impl AsMut<GameState> for Editor {
    fn as_mut(&mut self) -> &mut GameState {
        &mut self.game_state
    }
}

impl NewState for Editor {
    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {

        let camera = Rc::new(RefCell::new(String::default()));
        let map = level_map::Map::default();

        let camera_for_init = Rc::clone(&camera);
        let mut camera_for_free = Rc::clone(&camera);

        Self {
            camera: Rc::clone(&camera),
            game_state: GameState::new(name, schedule, active),
            initialize: Box::new(move |world, resources| {

                let camera_str = camera::initialize_camera(world);
                camera_for_init.replace(camera_str.clone());

                selection_box::initialize_selection_box(world, camera_str);
    
                resources.insert(map);    
                resources.insert(history::CurrentHistoricalStep::default());
                resources.insert(level_map::document::Document::default());
            }),
            free: Box::new(move |world, resources| {
                
                resources.remove::<level_map::document::Document>();
                resources.remove::<history::CurrentHistoricalStep>();

                camera::free_camera(world, &Rc::make_mut(&mut camera_for_free).borrow_mut());

                selection_box::free_all(world);
                map.free(world);
            })
        }
    }
}