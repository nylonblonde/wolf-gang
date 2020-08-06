use legion::*;
use crate::{
    game_state::{NewState, GameState, GameStateTraits},
    systems::{
        camera,
        history::History,
        level_map,
        selection_box,
    },
};

pub struct Editor {
    game_state: GameState,
    camera: String,
    map: level_map::Map,
}

impl GameStateTraits for Editor {
    fn initialize(&mut self, world: &mut World, resources: &mut Resources) {
        self.camera = camera::initialize_camera(world);
        // selection_box::initialize_selection_box(world, self.camera.clone());

        resources.insert(History::new());
        resources.insert(self.map);    
        resources.insert(level_map::document::Document::default());
    }
    fn free(&mut self, world: &mut World, resources: &mut Resources) {
        resources.remove::<History>();
        resources.remove::<level_map::document::Document>();

        camera::free_camera(world, &self.camera);
        // selection_box::free_all(world);
        self.map.free(world);
    }
    fn on_connection(&self, connection_id: u32, world: &mut World) {
        
        // Will need to check if this selection box belongs to the client before passing the camera name
        selection_box::initialize_selection_box(world, connection_id, Some(self.camera.clone()));

    }
}

impl AsMut<GameState> for Editor {
    fn as_mut(&mut self) -> &mut GameState {
        &mut self.game_state
    }
}

impl AsRef<GameState> for Editor {
    fn as_ref(&self) -> &GameState {
        &self.game_state
    }
}

impl NewState for Editor {
    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {

        Self {
            camera: String::default(),
            game_state: GameState::new(name, schedule, active),
            map: level_map::Map::default()
        }
    }
}