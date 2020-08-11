use legion::*;

use serde::{Serialize, Deserialize};

use crate::networking::UdpSocket;

use cobalt::{
    BinaryRateLimiter, 
    Client,
    ClientEvent,
    Config, 
    MessageKind, 
    NoopPacketModifier, 
    Server, 
    ServerEvent, 
    Socket,
};

use std::{
    collections::HashMap,
    io::{Error, ErrorKind},
    sync::{
        mpsc::TryRecvError,
    },
    net,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs},
};

use snap::raw::{Decoder, Encoder};
use bincode::{serialize, deserialize};

type Point = nalgebra::Vector3<i32>;

/// Resource used to store the client ID when it connects to a server so that we can know which entities belong to this client
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ClientID(u32);

impl ClientID {
    pub fn new(id: u32) -> Self {
        ClientID(id)
    }

    pub fn val(&self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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

pub trait Sender{
    fn get_message_type(&self) -> MessageType;
}

/// Struct used for sending messages from the client to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSender {
    pub data_type: DataType,
    pub message_type: MessageType
}

impl Sender for MessageSender{
    fn get_message_type(&self) -> MessageType {
        self.message_type
    }
}

/// Struct used for sending messages from the server to specific clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMessageSender {
    pub data_type: DataType,
    pub client_id: u32,
    pub message_type: MessageType
}

impl Sender for ServerMessageSender{
    fn get_message_type(&self) -> MessageType {
        self.message_type
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct NewConnection(u32);

impl NewConnection {
    pub fn new(id: u32) -> Self {
        NewConnection(id)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Disconnection(u32);

impl Disconnection {
    pub fn new(id: u32) -> Self {
        Disconnection(id)
    }
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
pub enum  DataType {
    NewConnection(crate::systems::networking::NewConnection),
    Disconnection(crate::systems::networking::Disconnection),
    MessageFragment(MessageFragment),
    MapInput(crate::systems::level_map::MapInput),
    MoveSelection{
        client_id: u32,
        point: Point
    },
}

pub fn create_server_system() -> impl systems::ParallelRunnable {

    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new();

    SystemBuilder::new("server_system")
        .with_query(<(Entity, Write<Server<UdpSocket, BinaryRateLimiter, NoopPacketModifier>>)>::query())
        .with_query(<(Entity, Read<ServerMessageSender>)>::query())
        .build(move |commands, world, _, queries| {

            let (server_query, messages_query) = queries;

            let messages = messages_query.iter(world)
                .map(|(entity, message_sender)| (*entity, (*message_sender).clone()))
                .collect::<Vec<(Entity, ServerMessageSender)>>();

            if let Some((entity, server)) = server_query.iter_mut(world).next() {
                
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
    
                            //Let everyone know this client has connected
                            for (_, conn) in server.connections() {
                                conn.send(MessageKind::Reliable, encoder.compress_vec(
                                    &bincode::serialize(&MessageSender{
                                        data_type: DataType::NewConnection(crate::systems::networking::NewConnection::new(id.0)),
                                        message_type: MessageType::Reliable
                                    }).unwrap()
                                ).unwrap());
                            }
                            
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
    
                            // Let everyone know this client has disconnected
                            for (_, conn) in server.connections() {
                                conn.send(MessageKind::Reliable, encoder.compress_vec(
                                    &bincode::serialize(&MessageSender{
                                        data_type: DataType::Disconnection(crate::systems::networking::Disconnection::new(id.0)),
                                        message_type: MessageType::Reliable
                                    }).unwrap()
                                ).unwrap());
                            }
    
                            if server.connections().len() == 0 {
                                println!("[Server] Closing out server as there are no more connections");
                                commands.remove(*entity);
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
                    }
                }
    
                messages.into_iter().for_each(|(entity, message)| {
                    let id = message.client_id;

                    let config = server.config();

                    if let Ok(conn) = server.connection(&cobalt::ConnectionID(id)) {
                        message_send_helper(conn, &message, &config, &mut encoder);
                    }

                    commands.remove(entity);
                });
 
                if let Ok(_) = server.send(false) {} //TODO: Honor send rate but not by sleeping the thread

            }
        })
}

pub fn create_client_system() -> impl systems::ParallelRunnable {
    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new();

    SystemBuilder::new("client_system")
        .with_query(<(Entity, Write<Client<UdpSocket, BinaryRateLimiter, NoopPacketModifier>>)>::query())
        .with_query(<(Entity, Read<MessageSender>)>::query())
        .build(move |commands, world, _, queries| {
            
            let (client_query, messages_query) = queries;

            let messages = messages_query.iter(world)
                .map(|(entity, message_sender)| (*entity, (*message_sender).clone()))
                .collect::<Vec<(Entity, MessageSender)>>();

            if let Some((entity, client)) = client_query.iter_mut(world).next() {
                // Accept incoming connections and fetch their events
                while let Ok(event) = client.receive() {
                    println!("{:?}", event);
                    // Handle events (e.g. Connection, Messages, etc.)
                    match event {
                        ClientEvent::Connection => {
                            let conn = client.connection().unwrap();
                            println!(
                                "[Client] Connection established ({}, {}ms rtt).",
                                conn.peer_addr(),
                                conn.rtt()
                            );

                            //TODO: push an entity to a world which will set the clientID resource
                            // let resources = resources.lock().unwrap();
                            // resources.insert(ClientID::new(conn.id().0));

                        },
                        ClientEvent::Message(message) => {
                            let conn = client.connection().unwrap();
                            println!(
                                "[Client] Message from server ({}, {}ms rtt)",
                                conn.peer_addr(),
                                conn.rtt(),
                            );

                            let payload = decoder.decompress_vec(&message).unwrap();
                            let data: DataType = deserialize(&payload).unwrap();

                            //TODO: try getting resources from the WolfGang assoc func or push an entity to handle it on the main thread
                            // let mut world = world.write().unwrap();
                            // let resources = resources.read().unwrap();

                            // match data {
                            //     DataType::MessageFragment(frag) => client_handle_fragments(frag, decoder, &mut message_fragments, &mut world, &resources),
                            //     _=> client_handle_data(data, &mut world, &resources)
                            // }
                        },
                        ClientEvent::ConnectionClosed(_) | ClientEvent::ConnectionLost(_) => {
                            let conn = client.connection().unwrap();
                            println!(
                                "[Client] ({}, {}ms rtt) disconnected.",
                                conn.peer_addr(),
                                conn.rtt()
                            );
                            
                            commands.remove(*entity);
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
                    messages.into_iter().for_each(|(entity, message)| {
                        message_send_helper(conn, &message, &config, &mut encoder);

                        commands.remove(entity);
                    });                        
                }

                // Send all outgoing messages.
                //
                // Also auto delay the current thread to achieve the configured tick rate.
                match client.send(false) { //TODO: Honor send rate but not by sleeping the thread
                    Ok(_) => {},
                    Err(err) => {
                        println!("{:?}", err);
                    }
                }
            }
            
        })
}

pub fn create_new_connection_thread_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {
    
    let mut query = <(Entity, Read<NewConnection>)>::query();
    
    Box::new(move |world, resources| {

        let results = query.iter(world)
            .map(|(entity, new_connection)| (*entity, *new_connection))
            .collect::<Vec<(Entity, NewConnection)>>();
        
        for (entity, new_connection) in results {
            crate::STATE_MACHINE.with(|s| {
                let state_machine = & *s.borrow();

                for state in &state_machine.states {
                    state.on_connection(new_connection.0, world, resources);
                }
            });

            //only need to act on a new connection once, get rid of the entity
            world.remove(entity);
        }
    })
}

pub fn create_disconnection_thread_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {
    
    let mut query = <(Entity, Read<Disconnection>)>::query();
    
    Box::new(move |world, resources| {

        let results = query.iter(world)
            .map(|(entity, disconnection)| (*entity, *disconnection))
            .collect::<Vec<(Entity, Disconnection)>>();
        
        for (entity, disconnection) in results {
            crate::STATE_MACHINE.with(|s| {
                let state_machine = & *s.borrow();

                for state in &state_machine.states {
                    state.on_disconnection(disconnection.0, world, resources);
                }
            });

            //only need to act on a disconnection once, get rid of the entity
            world.remove(entity);
        }
    })
}

fn client_handle_fragments(
    fragment: MessageFragment, 
    decoder: &mut Decoder, 
    message_fragments: &mut HashMap<u128, Vec<MessageFragment>>, 
    world: &mut World, 
    resources: &systems::SyncResources
) {

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
                                client_handle_data(data, world, resources);
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

fn client_handle_data(data: DataType, world: &mut World, resources: &systems::SyncResources) {
    match data {
        DataType::MapInput(r) => {
            if let Some(map) = resources.get::<crate::systems::level_map::Map>() {
                map.change(world, r.get_octree())
            }
        },  
        DataType::MoveSelection{client_id: id, point} => {

            use crate::systems::selection_box::MoveTo;

            //This may seem convoluded, but we only want messages to act on clients that were not the sender,
            // as their movement was already handled at the time the message was sent to avoid any perceived input
            // lag on their end. 
            if let Some(client_id) = resources.get::<ClientID>() {
                if id != client_id.0 {

                    world.push((
                        ClientID::new(id),
                        MoveTo(point)
                    ));

                }
            };

        },
        DataType::NewConnection(r) => {

            world.push(
                (r,)
            );
        },
        DataType::Disconnection(r) => {

            world.push(
                (r,)
            );
        }
        _ => {},
    }
}

fn message_send_helper<T>(
    connection: &mut cobalt::Connection<BinaryRateLimiter, NoopPacketModifier>, 
    message_sender: &T,
    config: &Config,
    encoder: &mut Encoder
) where T: Sender + serde::Serialize {

    let message = serialize(message_sender).unwrap().to_vec();
    let size = config.packet_max_size - std::mem::size_of::<MessageSender>();

    let payload = encoder.compress_vec(&message).unwrap();

    //fragment the payload if it is too large to send
    if payload.len() > size {

        let pieces = (payload.len() as f32 / size as f32).ceil() as usize;
        let uuid = uuid::Builder::from_slice(&payload[0..16]).unwrap().build().as_u128();

        for i in 0..pieces {

            let start = i * size;
            let end = std::cmp::min(payload.len(), start+size);

            connection.send(MessageKind::Reliable, encoder.compress_vec(
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
        connection.send(message_sender.get_message_type().as_kind(), payload);
    }
    
}