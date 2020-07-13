use crate::game_state::{GameState, GameStateTraits, NewState};
use legion::prelude::*;

use crate::systems::{
    udp::{ServerSocket, ClientSocket, BaseSocket}
};

pub struct Networking {
    game_state: GameState,
    free_func: Box<dyn FnMut(&mut World, &mut Resources)>,
    init_func: Box<dyn FnMut(&mut World, &mut Resources)>
}

impl GameStateTraits for Networking {

    fn initialize_func(&mut self) -> &mut Box<(dyn FnMut(&mut World, &mut Resources))> { 
        &mut self.init_func
    }

    fn free_func(&mut self) -> &mut Box<(dyn FnMut(&mut World, &mut Resources))> { 
        &mut self.free_func
    }
}

impl NewState for Networking {

    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {
        Self {
            game_state: GameState::new(
                name,
                schedule,
                active
            ),
            free_func: Box::new(|_,_|{}),
            init_func: Box::new(|_, resources|{
                
                resources.insert(ServerSocket(BaseSocket::new("255.255.255.255:0")));
                resources.insert(ClientSocket(BaseSocket::new("255.255.255.255:0")));

            })
        }
    }
}

impl AsMut<GameState> for Networking {

    fn as_mut(&mut self) -> &mut GameState { 
        &mut self.game_state
    }
}

impl AsRef<GameState> for Networking {

    fn as_ref(&self) -> &GameState { 
        &self.game_state
    }
}
