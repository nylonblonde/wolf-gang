use legion::prelude::*;
use crate::{
    game_state::{GameStateBase, GameStateTraits},
    history,
    systems::{
        camera,
        level_map,
        selection_box,
    },
};

pub struct Editor<'a> {
    name: &'static str,
    pub schedule: Schedule,
    active: bool,
    _phantom: std::marker::PhantomData<&'a ()>,
    map: level_map::Map,
    camera: String,
}

impl<'a> GameStateBase for Editor<'a> {}

impl<'a> GameStateTraits<'a> for Editor<'a> {

    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {
        Self {
            name,
            schedule,
            active,
            _phantom: std::marker::PhantomData,
            map: level_map::Map::default(),
            camera: String::default()
        }
    }

    fn initialize(&mut self, world: &mut World, resources: &mut Resources) {
        
        self.camera = camera::initialize_camera(world);
        selection_box::initialize_selection_box(world, self.camera.clone());

        resources.insert(self.map);    
        resources.insert(history::CurrentHistoricalStep::default());
        resources.insert(level_map::document::Document::default());

    }

    fn free(&mut self, world: &mut World, resources: &mut Resources) {
        
        resources.remove::<level_map::document::Document>();
        resources.remove::<history::CurrentHistoricalStep>();

        camera::free_camera(world, &self.camera);

        selection_box::free_all(world);
        self.map.free(world);
        
    }

}