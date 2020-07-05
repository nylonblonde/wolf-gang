use std::any::Any;
use legion::prelude::*;

use std::borrow::BorrowMut;

pub struct GameState<'a> {
    name: &'static str,
    pub schedule: Schedule,
    active: bool,
    _phantom: std::marker::PhantomData<&'a ()>
}

impl<'a> GameState<'a> {

    pub unsafe fn as_game_state<T: GameStateTraits<'a> + ?Sized>(original: &mut T) -> Box<GameState> {
        cast(original)
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
    
    pub fn get_name(&self) -> &'static str {
        self.name
    }

    pub fn as_any(&'a self) -> &(dyn Any + 'a) {
        self
    }

    pub fn as_any_mut(&'a mut self) -> &'a mut (dyn Any + 'a) {
        self
    }
}

pub trait GameStateTraits<'a>: GameStateBase {
    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self where Self: Sized;
    fn initialize(&mut self, world: &mut World, resources: &mut Resources);
    fn free(&mut self, world: &mut World, resources: &mut Resources);
}

unsafe fn cast<T, U: ?Sized>(original: &mut U) -> Box<T> {

    let ptr: *mut U = original;
    let new: *mut T = ptr as *mut T;
    let new = new.clone();

    Box::from_raw(new)
}

pub trait GameStateBase:  {} 

impl<'a> GameStateBase for GameState<'a> {}

impl<'a> GameStateTraits<'a> for GameState<'a> {

    fn initialize(&mut self, _: &mut World, _: &mut Resources) {}
    fn free(&mut self, _: &mut World, _: &mut Resources) {}

    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {
        GameState {
            name,
            schedule,
            active,
            _phantom: std::marker::PhantomData
        }
    }
}

impl<'a> AsMut<Schedule> for GameState<'a> {
    fn as_mut(&mut self) -> &mut Schedule {
        &mut self.schedule
    }
}

pub struct StateMachine<'a> {
    pub states: Vec<Box<dyn GameStateTraits<'a>>>
}

impl<'a> StateMachine<'a> {

    pub const STATES: Vec<Box<dyn GameStateTraits<'a>>> = Vec::new();

    pub fn add_state(&mut self, mut game_state: impl GameStateTraits<'a> + 'static, world: &mut legion::world::World, resources: &mut Resources) -> &Box<dyn GameStateTraits<'a>> {

        game_state.initialize(world, resources);

        self.states.push(Box::new(game_state));

        &self.states.last().unwrap()
    }

    pub fn set_state_active(&'a mut self, name: &'static str, active: bool) {
        
        for state in &mut self.states.iter_mut() {
            let mut state = state.as_mut();

            let mut state: Box<GameState> = unsafe { cast(state) };

            let state: &mut GameState = state.borrow_mut();

            // if let Some(state) = state.as_mut() {
                if state.get_name() == name {
                    state.set_active(active);
                }
            // };
        }

    }
}

