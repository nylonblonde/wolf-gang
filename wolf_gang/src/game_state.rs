use legion::*;

use std::{
    cell::RefCell,
};

pub struct GameState {
    name: &'static str,
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

pub struct BasicGameState {
    game_state: GameState
}

impl GameStateTraits for BasicGameState {}

impl AsMut<GameState> for BasicGameState {
    fn as_mut(&mut self) -> &mut GameState {
        &mut self.game_state
    }
}

impl AsRef<GameState> for BasicGameState {
    fn as_ref(&self) -> &GameState {
        &self.game_state
    }
}

impl NewState for BasicGameState {
    fn new(name: &'static str, active: bool) -> Self {

        Self {
            game_state: GameState::new(name, active),
        }
    }
}

pub trait GameStateTraits: NewState + AsMut<GameState> + AsRef<GameState> {
    fn initialize(&mut self, _: &mut World, _: &mut Resources) {}
    fn free(&mut self, _: &mut World, _: &mut Resources) {}
    fn on_connection(&self, _connection_id: u32, _world: &mut World, _resources: &mut Resources) {}
    fn on_disconnection(&self, _connection_id: u32, _world: &mut World, _resources: &mut Resources) {}
    /// Allows us to define a method for the server to call when a client connects. This would typically be used to
    /// communicate established data to the newly connected client, making things like join-in-progress possible. 
    /// Since this needs to be processed on the main thread, a MessageBatch entity needs to be pushed
    /// which will populate the server's message pool
    fn on_client_connected(&self, _connection_id: u32, _world: &mut World, _resources: &mut Resources) {}
}

pub trait NewState {
    fn new(name: &'static str, active: bool) -> Self where Self: Sized;
}

impl NewState for GameState {  
    fn new(name: &'static str, active: bool) -> Self {
        GameState {
            name,
            // schedule: RefCell::new(schedule),
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

pub struct Schedules {
    schedules: Vec<(&'static str, RefCell<Schedule>)>,
}

impl Schedules {
    fn new() -> Self {
        Schedules {
            schedules: Vec::new()
        }
    }

    pub fn get(&self, name: &'static str) -> Option<&RefCell<Schedule>> {
        self.schedules.iter().find(|(n, _)| n == &name).map(|(_, sched)| sched)
    }

    pub fn push(&mut self, item: (&'static str, RefCell<Schedule>)) {
        self.schedules.push(item);
    }
}

pub struct StateMachine {
    states: Vec<RefCell<Box<dyn GameStateTraits>>>,
    schedules: Schedules
}

impl Default for StateMachine {
    fn default() -> Self {
        StateMachine {
            states: Vec::new(),
            schedules: Schedules::new(),
        }    
    }
}

impl StateMachine {

    pub fn add_state(&mut self, mut game_state: impl GameStateTraits + 'static, schedule: Schedule, world: &mut World, resources: &mut Resources) -> &RefCell<Box<dyn GameStateTraits>> {

        game_state.initialize(world, resources);

        self.schedules.push((game_state.as_ref().get_name(), RefCell::new(schedule)));

        self.states.push(RefCell::new(Box::new(game_state)));
        self.states.last().unwrap()
    }

    pub fn get_states(&self) -> &Vec<RefCell<Box<dyn GameStateTraits>>> {
        &self.states
    }

    pub fn get_state(&self, name: &'static str) -> Option<&RefCell<Box<dyn GameStateTraits>>> {

        for state in &self.states {

            let borrowed = state.borrow();

            let game_state: &GameState = borrowed.as_ref().as_ref();
            if game_state.get_name() == name {
                return Some(state);
            }
        }

        None
    }

    pub fn get_schedule(&self, name: &'static str) -> Option<&RefCell<Schedule>> {
        self.schedules.get(name)
    }

    pub fn set_state_active(&self, name: &'static str, active: bool) {
        
        for state in &self.states {
            let mut borrowed = state.borrow_mut();
            let state = borrowed.as_mut();

            let game_state: &mut GameState = state.as_mut();

            if game_state.get_name() == name {
                game_state.set_active(active);
            }
        }

    }
}

