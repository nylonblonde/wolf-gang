use std::collections::HashMap;
use crate::game_state::{GameState, GameStateTraits, NewState};
use legion::prelude::*;

use cobalt::{
    BinaryRateLimiter, 
    Client,
    ClientEvent,
    Config, 
    MessageKind, 
    NoopPacketModifier, 
    Server, 
    ServerEvent, 
    UdpSocket,
};

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use bincode::{deserialize, serialize};

pub struct ClientAddr(pub SocketAddr);
pub struct ServerAddr(pub SocketAddr);

impl Default for ClientAddr {
    fn default() -> Self {
        Self("127.0.0.1:1234".parse().unwrap())
    }
}

impl Default for ServerAddr {
    fn default() -> Self {
        Self("0.0.0.0:1234".parse().unwrap())
    }
}

use serde_derive::{Deserialize, Serialize};

type Point = nalgebra::Vector3<i32>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum  DataType {
    MapInsert(crate::systems::level_map::MapInsert),
    MapRemove(crate::systems::level_map::MapRemove)
}

#[derive(Debug, Clone)]
pub struct MessageSender {
    pub data_type: DataType,
    pub message_kind: MessageKind
}

pub struct MessagePool {
    pub messages: Vec<MessageSender>
}

pub struct Networking {
    game_state: GameState,
    free_func: Box<dyn FnMut(&mut World, &mut Resources)>,
    init_func: Box<dyn FnMut(&mut World, &mut Resources)>,
}

impl GameStateTraits for Networking {

    fn initialize_func(&mut self) -> &mut Box<(dyn FnMut(&mut World, &mut Resources))> { 
        &mut self.init_func
    }

    fn free_func(&mut self) -> &mut Box<(dyn FnMut(&mut World, &mut Resources))> { 
        &mut self.free_func
    }
}

fn client_process(client: &mut Client<UdpSocket, BinaryRateLimiter, NoopPacketModifier>, send_wait: bool) -> bool {
    // Accept incoming connections and fetch their events
    while let Ok(event) = client.receive() {
        // Handle events (e.g. Connection, Messages, etc.)
        match event {
            ClientEvent::Connection => {
                let conn = client.connection().unwrap();
                println!(
                    "[Client] Connection established ({}, {}ms rtt).",
                    conn.peer_addr(),
                    conn.rtt()
                );

            },
            ClientEvent::Message(message) => {
                let conn = client.connection().unwrap();
                println!(
                    "[Client] Message from server ({}, {}ms rtt): {:?}",
                    conn.peer_addr(),
                    conn.rtt(),
                    message
                );

            },
            ClientEvent::ConnectionClosed(_) | ClientEvent::ConnectionLost(_) => {
                let conn = client.connection().unwrap();
                println!(
                    "[Client] ({}, {}ms rtt) disconnected.",
                    conn.peer_addr(),
                    conn.rtt()
                );
                return false
            },
            _ => {}
        }
    }

    if let Ok(conn) = client.connection() {

        let mut game_lock = crate::GAME_UNIVERSE.lock().unwrap();
        let game = &mut *game_lock;

        let resources = &mut game.resources;

        let mut message_pool = resources.get_mut::<MessagePool>().unwrap();

        if message_pool.messages.len() > 0 {
            for message_sender in &message_pool.messages {
                conn.send(message_sender.message_kind, serialize(&message_sender.data_type).unwrap().to_vec());
            }
            println!("Sending the message to the listener");

            message_pool.messages.drain(..);
        }
    }

    // Send all outgoing messages.
    //
    // Also auto delay the current thread to achieve the configured tick rate.
    match client.send(send_wait) {
        Ok(_) => {},
        Err(err) => {
            println!("{:?}", err);
        }
    }

    true
}

impl NewState for Networking {

    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {

        Self {
            game_state: GameState::new(
                name,
                schedule,
                active
            ),
            free_func: Box::new(move |_,_|{}),
            init_func: Box::new(move |_, resources|{
                
                resources.insert(MessagePool{
                    messages: Vec::new()
                });

                let server_addr = resources.get_mut_or_default::<ServerAddr>().unwrap().0;

                let mut config = Config{
                    connection_init_threshold: std::time::Duration::from_millis(1000),
                    ..Default::default()
                };

                std::thread::spawn(move || {
                    let mut server = Server::<UdpSocket, BinaryRateLimiter, NoopPacketModifier>::new(
                        config.clone()
                    );
                    server.listen(server_addr).expect("Failed to bind to socket.");

                    'server: loop {

                        let mut data_vec: Vec<DataType> = Vec::new();

                        while let Ok(event) = match server.accept_receive() {
                            Ok(r) => Ok(r),
                            Err(err) => {
                                
                                // println!("{:?}", err);
                                Err(err)
                            }
                        } {
                            match event {
                                ServerEvent::Connection(id) => {
                                    let conn = server.connection(&id).unwrap();
                                    println!(
                                        "[Server] Client {} ({}, {}ms rtt) connected.",
                                        id.0,
                                        conn.peer_addr(),
                                        conn.rtt()
                                    );
            
                                },
                                ServerEvent::Message(id, message) => {
                                    let conn = server.connection(&id).unwrap();
                                    println!(
                                        "[Server] Message from client {} ({}, {}ms rtt): {:?}",
                                        id.0,
                                        conn.peer_addr(),
                                        conn.rtt(),
                                        message
                                    );

                                    data_vec.push(deserialize(&message).unwrap());
            
                                },
                                ServerEvent::ConnectionClosed(id, _) | ServerEvent::ConnectionLost(id, _) => {
                                    let conn = server.connection(&id).unwrap();
                                    println!(
                                        "[Server] Client {} ({}, {}ms rtt) disconnected.",
                                        id.0,
                                        conn.peer_addr(),
                                        conn.rtt()
                                    );
                                    break 'server;
                                },
                                _ => {
                                }
                            }
                        }

                        if data_vec.len() > 0 {
                            let mut game_lock = crate::GAME_UNIVERSE.lock().unwrap();
                            let game = &mut *game_lock;

                            let resources = &mut game.resources;
                            let world = &mut game.world;

                            for data in data_vec {
                                match data {
                                    DataType::MapInsert(r) => {
                                        r.execute(world, resources);
                                    },
                                    DataType::MapRemove(r) => {
                                        r.execute(world, resources);
                                    }
                                }
                            }
                        }

                        if let Ok(_) = server.send(true) {}
                    }

                    server.shutdown().unwrap();
                });
            
                let client_addr = resources.get_or_default::<ClientAddr>().unwrap();

                let addr = client_addr.0;
                let ip = addr.ip();

                if !ip.is_global() {

                    config.send_rate = 1000;
                    let mut client = Client::new(config);

                    std::thread::spawn(move || {
                        client.connect(addr).expect("Couldn't connect to local address!");
                        'client: loop {
                            if !client_process(&mut client, true) {
                                break 'client;
                            }
                        }
                        client.disconnect().ok();
                    });

                } else {

                    let mut client = Client::new(config);

                    std::thread::spawn(move ||{

                        client.connect(addr).expect("Couldn't connect to global address!");

                        'client: loop {
                            if !client_process(&mut client, true) {
                                break 'client;
                            }
                        }
                        client.disconnect().ok();
                    });
                }
                

                
            }),
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
