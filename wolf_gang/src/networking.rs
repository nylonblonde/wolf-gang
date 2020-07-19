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

use snap::raw::{Decoder, Encoder};

use bincode::{deserialize, serialize};

use std::sync::mpsc;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub struct ClientAddr(pub SocketAddr);
pub struct ServerAddr(pub SocketAddr);

impl Default for ClientAddr {
    fn default() -> Self {
        Self("127.0.0.1:1234".parse().unwrap())
    }
}

impl Default for ServerAddr {
    fn default() -> Self {
        Self("127.0.0.1:1234".parse().unwrap())
    }
}

use serde_derive::{Deserialize, Serialize};

type Point = nalgebra::Vector3<i32>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum  DataType {
    MessageFragment(MessageFragment),
    MapInput(crate::systems::level_map::MapInput)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFragment {
    //UUID of MessageFragment held collection
    uuid: u128,
    //id position of the fragment
    id: usize,
    //size of each fragment - need this because slices at the end may be shorter
    size: usize,
    //how many pieces the message has been split into
    pieces: usize,
    payload: Vec<u8>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
//Have to do this because cobalt::MessageKind doesn't implement serialize, deserialize. 
pub enum MessageType {
    Instant,
    Reliable,
    Ordered,
}

impl MessageType {
    /// Returns the related cobalt::MessageKind
    fn as_kind(&self) -> MessageKind {
        match self {
            Self::Instant => MessageKind::Instant,
            Self::Ordered => MessageKind::Ordered,
            Self::Reliable => MessageKind::Reliable
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSender {
    pub data_type: DataType,
    pub message_type: MessageType
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

impl NewState for Networking {

    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {

        Self {
            game_state: GameState::new(
                name,
                schedule,
                active
            ),
            free_func: Box::new(move |_,_|{
                
            }),
            init_func: Box::new(move |_, resources|{
                
                resources.insert(MessagePool{
                    messages: Vec::new()
                });

                let server_addr = resources.get_mut_or_default::<ServerAddr>().unwrap().0;

                let mut config = Config{
                    message_quota_ordered: 80.,
                    message_quota_instant: 5.,
                    message_quota_reliable: 15.,
                    packet_drop_threshold: std::time::Duration::from_secs(3),
                    connection_drop_threshold: std::time::Duration::from_secs(15),
                    connection_init_threshold: std::time::Duration::from_millis(1000),
                    ..Default::default()
                };

                let client_addr = resources.get_or_default::<ClientAddr>().unwrap();

                let addr = client_addr.0;
                let ip = addr.ip();

                if !ip.is_global() {
                    config.send_rate = 1000;
                }

                std::thread::spawn(move || {
                    let mut encoder = Encoder::new();
                    let mut decoder = Decoder::new();

                    let mut server = Server::<UdpSocket, BinaryRateLimiter, NoopPacketModifier>::new(
                        config.clone()
                    );
                    server.listen(server_addr).expect("Failed to bind to socket.");

                    'server: loop {

                        while let Ok(event) = server.accept_receive() {
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
                                        "[Server] Message from client {} ({}, {}ms rtt)",
                                        id.0,
                                        conn.peer_addr(),
                                        conn.rtt(),
                                    );

                                    let decompressed = decoder.decompress_vec(&message).unwrap();
                                    let message: MessageSender = deserialize(&decompressed).unwrap();
                                    let payload = encoder.compress_vec(&serialize(&message).unwrap()).unwrap();

                                    // Send a message to all connected clients
                                    for (_, conn) in server.connections() {
                                        conn.send(message.message_type.as_kind(), payload.clone());
                                    }
            
                                },
                                ServerEvent::ConnectionClosed(id, _) | ServerEvent::ConnectionLost(id, _) => {
                                    let conn = server.connection(&id).unwrap();
                                    println!(
                                        "[Server] Client {} ({}, {}ms rtt) disconnected.",
                                        id.0,
                                        conn.peer_addr(),
                                        conn.rtt()
                                    );
                                    if server.connections().len() == 0 {
                                        println!("[Server] Closing out server as there are no more connections");
                                        break 'server;
                                    }
                                },
                                ServerEvent::PacketLost(id, _) => {
                                    let conn = server.connection(&id).unwrap();
                                    println!(
                                        "[Server] Packet dropped {} ({}, {}ms rtt)",
                                        id.0,
                                        conn.peer_addr(),
                                        conn.rtt(),
                                    );
                                },
                                ServerEvent::ConnectionCongestionStateChanged(id, _) => {
                                    let conn = server.connection(&id).unwrap();
                                    println!(
                                        "[Server] Congestion State Changed {} ({}, {}ms rtt)",
                                        id.0,
                                        conn.peer_addr(),
                                        conn.rtt()
                                    );
                                }
                                _ => {}
                            }
                        }

                        if let Ok(_) = server.send(true) {}
                    }

                    server.shutdown().unwrap();
                });

                let mut client: Client<UdpSocket, BinaryRateLimiter, NoopPacketModifier> = Client::new(config);

                std::thread::spawn(move ||{

                    let mut message_fragments: HashMap::<u128, Vec<MessageFragment>> = HashMap::new();
                    let mut encoder = Encoder::new();
                    let mut decoder = Rc::new(Decoder::new());

                    client.connect(addr).expect("Couldn't connect to global address!");

                    'client: loop {
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
                                        "[Client] Message from server ({}, {}ms rtt)",
                                        conn.peer_addr(),
                                        conn.rtt(),
                                    );

                                    let decoder = Rc::get_mut(&mut decoder).unwrap();

                                    let payload = decoder.decompress_vec(&message).unwrap();
                                    let data: DataType = deserialize(&payload).unwrap();

                                    // if let data = DataType::ManuallyDisconnect {
                                    //     break 'client;
                                    // }

                                    // if let data = DataType::MessageFragment {
                                    //     client_handle_fragments(data, &mut message_fragments);
                                    // }

                                    match data {
                                        DataType::MessageFragment(frag) => client_handle_fragments(frag, decoder, &mut message_fragments),
                                        _=> client_handle_data(data)
                                    }
                                },
                                ClientEvent::ConnectionClosed(_) | ClientEvent::ConnectionLost(_) => {
                                    let conn = client.connection().unwrap();
                                    println!(
                                        "[Client] ({}, {}ms rtt) disconnected.",
                                        conn.peer_addr(),
                                        conn.rtt()
                                    );
                                    break 'client
                                },
                                ClientEvent::PacketLost(_) => {
                                    let conn = client.connection().unwrap();
                                    println!(
                                        "[Client] ({}, {}ms rtt) Packet lost",
                                        conn.peer_addr(),
                                        conn.rtt(),
                                    );
                                },
                                ClientEvent::ConnectionCongestionStateChanged(_) => {
                                    let conn = client.connection().unwrap();
                                    println!(
                                        "[Client] Congestion State Changed ({}, {}ms rtt)",
                                        conn.peer_addr(),
                                        conn.rtt()
                                    );
                                }
                                _ => {}
                            }
                        }

                        let config = client.config();

                        if let Ok(conn) = client.connection() {

                            let mut game_lock = crate::GAME_UNIVERSE.lock().unwrap();
                            let game = &mut *game_lock;

                            let resources = &mut game.resources;

                            let mut message_pool = resources.get_mut::<MessagePool>().unwrap();

                            if message_pool.messages.len() > 0 {

                                for message_sender in &message_pool.messages {

                                    let message = serialize(&message_sender).unwrap().to_vec();
                                    let size = config.packet_max_size - std::mem::size_of::<MessageSender>();

                                    let payload = encoder.compress_vec(&message).unwrap();

                                    //fragment the payload if it is too large to send
                                    if payload.len() > size {
                                        println!("[Client] payload is too large to send");

                                        let pieces = (payload.len() as f32 / size as f32).ceil() as usize;
                                        let uuid = uuid::Builder::from_slice(&payload[0..16]).unwrap().build().as_u128();

                                        for i in 0..pieces {

                                            let start = i * size;
                                            let end = std::cmp::min(payload.len(), start+size);

                                            conn.send(MessageKind::Reliable, encoder.compress_vec(
                                                &serialize(&MessageSender{
                                                    data_type: DataType::MessageFragment(MessageFragment{
                                                        uuid,
                                                        id: i,
                                                        pieces,
                                                        size,
                                                        payload: payload[start..end].to_vec()
                                                    }),
                                                    message_type: MessageType::Reliable
                                                })
                                                .unwrap().to_vec()
                                            ).unwrap());
                                        }

                                    } else {
                                        conn.send(message_sender.message_type.as_kind(), payload);
                                    }
                                }
                                message_pool.messages.drain(..);
                            }
                        }

                        // Send all outgoing messages.
                        //
                        // Also auto delay the current thread to achieve the configured tick rate.
                        match client.send(true) {
                            Ok(_) => {},
                            Err(err) => {
                                println!("{:?}", err);
                            }
                        }
                    }

                    client.disconnect().ok();
                });
                
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

fn client_handle_fragments(fragment: MessageFragment, decoder: &mut Decoder, message_fragments: &mut HashMap<u128, Vec<MessageFragment>>) {

    match message_fragments.get_mut(&fragment.uuid) {
        Some(frag_vec) => {

            frag_vec.sort_by(|a, b| a.id.cmp(&b.id));

            let MessageFragment {
                size,
                pieces,
                uuid,
                ..
            } = fragment;

            frag_vec.push(fragment);

            //If we've received all of the pieces
            if frag_vec.len() == pieces {

                //reconstruct the fragmented data
                let mut combined: Vec<u8> = Vec::with_capacity(size * frag_vec.len());

                for frag in frag_vec {
                    combined.extend(frag.payload.iter());
                }

                //Once we're done, remove the key from message fragments
                message_fragments.remove(&uuid);

                match decoder.decompress_vec(&combined) {

                    Ok(payload) => {

                        //if it is able to succesfully reconstruct the data, handle that data
                        match deserialize::<DataType>(&payload) {
                            Ok(data) => {
                                println!("[Client] Succesfully reconstructed data from fragments");
                                client_handle_data(data);
                            },
                            Err(_) => println!("[Client] Unable to reconstruct data from fragments")
                        }
                    },

                    Err(err) => println!("[Client] Failed to decompress fragments' payload with error: {:?}", err)
                }

            }
        },
        None => {
            message_fragments.insert(fragment.uuid, vec![fragment]);
        }
    }
}

fn client_handle_data(data: DataType) {
    match data {
        DataType::MapInput(r) => {

            let mut game_lock = crate::GAME_UNIVERSE.lock().unwrap();
            let game = &mut *game_lock;

            let resources = &mut game.resources;
            let world = &mut game.world;

            r.execute(world, resources)
        },
        _ => {},
    }
}
