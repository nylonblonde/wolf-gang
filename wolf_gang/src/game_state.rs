use std::ops::Deref;
use std::any::Any;
use legion::prelude::*;

use std::borrow::Borrow;

pub struct GameState {
    name: &'static str,
    pub schedule: Schedule,
    active: bool,
}

impl GameState{

    // pub unsafe fn as_game_state<T: GameStateTraits<'a> + ?Sized>(original: &mut T) -> Box<GameState> {
    //     cast(original)
    // }

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

pub trait GameStateTraits: NewState + AsMut<GameState> {
    fn initialize_func(&mut self) -> &mut Box<dyn FnMut(&mut World, &mut Resources)>;
    fn free_func(&mut self) -> &mut Box<dyn FnMut(&mut World, &mut Resources)>;
}

pub trait NewState {
    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self where Self: Sized;
}

impl NewState for GameState {  
    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {
        GameState {
            name,
            schedule,
            active,
        }
    }
}

impl AsMut<Schedule> for GameState {
    fn as_mut(&mut self) -> &mut Schedule {
        &mut self.schedule
    }
}

impl AsMut<GameState> for GameState {
    fn as_mut(&mut self) -> &mut GameState {
        self
    }
}

pub struct StateMachine {
    pub states: Vec<Box<dyn GameStateTraits>>
}

impl StateMachine {

    pub fn add_state(&mut self, mut game_state: impl GameStateTraits + 'static, world: &mut legion::world::World, resources: &mut Resources) -> &Box<dyn GameStateTraits> {

        game_state.initialize_func()(world, resources);

        self.states.push(Box::new(game_state));

        self.states.last().unwrap()
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

