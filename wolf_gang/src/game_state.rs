use legion::*;

use std::cell::RefCell;

pub struct GameState {
    name: &'static str,
    pub schedule: RefCell<Schedule>,
    active: bool,
}

impl GameState{

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    
    pub fn get_name(&self) -> &'static str {
        self.name
    }
}
pub trait GameStateTraits: NewState + AsMut<GameState> + AsRef<GameState> {
    fn initialize(&mut self, _: &mut World, _: &mut Resources) {}
    fn free(&mut self, _: &mut World, _: &mut Resources) {}
    fn on_connection(&self, _connection_id: u32, _world: &mut World) {}
}

pub trait NewState {
    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self where Self: Sized;
}

impl NewState for GameState {  
    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {
        GameState {
            name,
            schedule: RefCell::new(schedule),
            active,
        }
    }
}

impl AsMut<GameState> for GameState {
    fn as_mut(&mut self) -> &mut GameState {
        self
    }
}

impl AsRef<GameState> for GameState {
    fn as_ref(&self) -> &GameState {
        self
    }
}

pub struct StateMachine {
    pub states: Vec<Box<dyn GameStateTraits>>
}

impl StateMachine {

    pub fn add_state(&mut self, mut game_state: impl GameStateTraits + 'static, world: &mut legion::world::World, resources: &mut Resources) -> &Box<dyn GameStateTraits> {

        game_state.initialize(world, resources);

        self.states.push(Box::new(game_state));

        self.states.last().unwrap()
    }

    pub fn get_state(&self, name: &'static str) -> Option<&Box<dyn GameStateTraits>> {

        for state in &self.states {

            let game_state: &GameState = state.as_ref().as_ref();
            if game_state.get_name() == name {
                return Some(state);
            }
        }

        None
    }

    pub fn get_state_mut(&mut self, name: &'static str) -> Option<&mut Box<dyn GameStateTraits>> {

        for state in &mut self.states {

            let game_state: &GameState = state.as_mut().as_mut();
            if game_state.get_name() == name {
                return Some(state);
            }
        }

        None
    }


    pub fn set_state_active(&mut self, name: &'static str, active: bool) {
        
        for state in &mut self.states.iter_mut() {
            let state = state.as_mut();

            let game_state: &mut GameState = state.as_mut();

            if game_state.get_name() == name {
                game_state.set_active(active);
            }
        }

    }
}

