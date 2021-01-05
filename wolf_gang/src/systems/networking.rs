use legion::*;

use serde::{Serialize, Deserialize};

use crate::{
    networking,
    networking::UdpSocket,
};

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
    net,
    net::SocketAddr,
    time::{
        Duration, Instant
    },
};

use snap::raw::{Decoder, Encoder};
use bincode::{serialize, deserialize};

type Point = nalgebra::Vector3<i32>;
type AABB = crate::geometry::aabb::AABB<i32>;

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

impl Default for ClientID {
    fn default() -> ClientID {
        ClientID(0)
    }
}

/// Component that gets used to set the ClientID resource on the main thread
#[derive(Copy, Clone)]
pub struct SetClientID {
    client_id: ClientID
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

/// The inner value represents the client id of the new connection
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct NewConnection(u32);

impl NewConnection {
    pub fn new(id: u32) -> Self {
        NewConnection(id)
    }
}

/// The inner value represents the client id of the disconnection
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Disconnection(u32);

impl Disconnection {
    pub fn new(id: u32) -> Self {
        Disconnection(id)
    }
}

/// Component which belongs to the entity that will be calling the game state's on_client_connected method, which
/// is responsible for handling logic when a new client connects to the server, ususally meant for sending data from
/// the server to that new connection.
#[derive(Debug, Copy, Clone)]
pub struct OnClientConnected(u32);

impl OnClientConnected {
    pub fn new(id: u32) -> Self {
        OnClientConnected(id)
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
    /// Message sent by the server to communicate the existence of other selection boxes when a new client connects
    CreateSelectionBox{
        client_id: u32,
        box_type: crate::systems::selection_box::ToolBoxType,
        active: bool,
        coord_pos: Point,
        aabb: AABB
    },
    ActivateTerrainToolBox{
        client_id: u32
    },
    ActivateActorToolBox {
        client_id: u32
    },
    ActorToolSelection {
        client_id: u32,
        actor_id: u32
    },
    ///Handles changes to actors such as insertion or removal. Edits to existing actors are handled through insertion but is checked against by the uuid
    ActorChange{
        change: crate::systems::actor::ActorChange,
        store_history: Option<u32>
    },
    MapInput(crate::collections::octree::Octree<i32, crate::systems::level_map::TileData>),
    ///Handles changes to map like insertion, removal, cutting, pasting, takes an optional u32 as store_history to store the change in the history for that client_id if need be
    MapChange{
        change: crate::systems::level_map::MapChange,
        store_history: Option<u32>
    },
    MapNew,
    HistoryStep{
        amount: i32,
        client_id: u32,
    },
    /// Handles movement and expansion of selection boxes since the selection box moves when expanded anyway
    UpdateSelectionBounds{
        client_id: u32,
        coord_pos: Point,
        aabb: AABB
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

                            //create an entity that will call on_client_connected
                            commands.push(
                                (
                                    OnClientConnected(id.0),
                                )
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
 
                server.send(false).ok(); //TODO: Honor send rate but not by sleeping the thread

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

                            commands.push(
                                (
                                    SetClientID{
                                        client_id: ClientID::new(conn.id().0)
                                    },
                                )
                            );

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

                            //Create data entities to handle them on the main thread
                            commands.push(
                                (data,)
                            );
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
                client.send(false).ok(); //TODO: Honor send rate but not by sleeping the thread
                
            }
            
        })
}

pub fn create_client_multicast_connection_system() -> impl systems::Runnable {
    let mut last_sent = Instant::now();
    let mut wait_for = Duration::from_millis(0);

    SystemBuilder::new("client_multicast_connection_system")
        .with_query(<(Entity, Write<std::net::UdpSocket>, Write<Client<networking::UdpSocket, BinaryRateLimiter, NoopPacketModifier>>)>::query())
        .build(move |commands, world, _, query| {

            if let Some((entity, socket, client)) = query.iter_mut(world).next() {

                if Instant::now() - last_sent > wait_for {

                    println!("Sending an IP request to {:?}", networking::MULTICAST_ADDR_V4);
                    //8008 will be interpreted as an IP request
                    socket.send_to(&[8,0,0,8], networking::MULTICAST_ADDR_V4).unwrap();
                    last_sent = Instant::now();
                    wait_for = Duration::from_secs(5);

                }

                let mut buffer = [0; 4];
                // This is kind of sketchy because we don't check the validity of the origin of the request, maybe a random packet 
                // should be sent with the request and returned to be verified

                if let Ok((_, src_addr)) = socket.recv_from(&mut buffer) {

                    client.connect(src_addr).expect("Couldn't connect to local address!");
                    client.socket().unwrap().connect_multicast(networking::MULTICAST_ADDR_V4.parse::<SocketAddr>().unwrap()).unwrap();
                    println!("[Client] {:?} Connecting to {:?}...", client.socket().unwrap().local_addr().unwrap(), client.peer_addr().unwrap());

                    wait_for = Duration::from_millis(0);

                    commands.remove_component::<net::UdpSocket>(*entity);
                }

            }
        })
}

pub fn create_data_handler_threal_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {

    let mut query = <(Entity, Read<DataType>)>::query();
    let mut message_fragments: HashMap<u128, Vec<MessageFragment>> = HashMap::new();
    let mut decoder = Decoder::new();

    Box::new(move |world, resources| {

        let entities = query.iter(world)
            .map(|(entity, data)| (*entity, (*data).clone()))
            .collect::<Vec<(Entity, DataType)>>();

        entities.into_iter().for_each(|(entity, data_type)| {

            match data_type {
                DataType::MessageFragment(frag) => client_handle_fragments(frag, &mut decoder, &mut message_fragments, world, resources),
                _=> client_handle_data(data_type, world, resources)
            }

            world.remove(entity);
        });

    })
}

pub fn create_set_client_id_thread_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {

    let mut query = <(Entity, Read<SetClientID>)>::query();

    Box::new(move |world, resources| {

        let entities = query.iter(world)
            .map(|(entity, client_id)| (*entity, *client_id))
            .collect::<Vec<(Entity, SetClientID)>>();

        entities.into_iter().for_each(|(entity, set_client_id)| {
            resources.insert(set_client_id.client_id);

            world.remove(entity);
        })

    })
}

pub fn create_new_connection_thread_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {
    
    let mut query = <(Entity, Read<NewConnection>)>::query();
    
    Box::new(move |world, resources| {

        let results = query.iter(world)
            .map(|(entity, new_connection)| (*entity, *new_connection))
            .collect::<Vec<(Entity, NewConnection)>>();
        
            results.into_iter().for_each(|(entity, connection)| {
                crate::STATE_MACHINE.with(|s| {
                    let state_machine = & *s.borrow();
    
                    state_machine.get_states().iter().for_each(|state| {
                        state.borrow().on_connection(connection.0, world, resources);
                    });
                });
    
                //only need to act on a connection once, get rid of the entity
                world.remove(entity);
            });
    })
}

pub fn create_disconnection_thread_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {
    
    let mut query = <(Entity, Read<Disconnection>)>::query();
    
    Box::new(move |world, resources| {

        let results = query.iter(world)
            .map(|(entity, disconnection)| (*entity, *disconnection))
            .collect::<Vec<(Entity, Disconnection)>>();
        
        results.into_iter().for_each(|(entity, disconnection)| {
            crate::STATE_MACHINE.with(|s| {
                let state_machine = & *s.borrow();

                state_machine.get_states().iter().for_each(|state| {
                    state.borrow().on_disconnection(disconnection.0, world, resources);
                });
            });

            //only need to act on a disconnection once, get rid of the entity
            world.remove(entity);
        });
    })
}

pub fn create_on_client_connection_thread_local_fn() -> Box<dyn FnMut(&mut World, &mut Resources)> {

    let mut query = <(Entity, Read<OnClientConnected>)>::query();

    Box::new(move |world, resources| {

        let results = query.iter(world)
            .map(|(entity, on_connection)| (*entity, *on_connection))
            .collect::<Vec<(Entity, OnClientConnected)>>();

        results.into_iter().for_each(|(entity, on_connected)| {

            crate::STATE_MACHINE.with(|s| {
                let state_machine = & *s.borrow();

                state_machine.get_states().iter().for_each(|state| {
                    state.borrow().on_client_connected(on_connected.0, world, resources);
                })
            });

            world.remove(entity);
        })

    })
}

fn client_handle_fragments(
    fragment: MessageFragment, 
    decoder: &mut Decoder, 
    message_fragments: &mut HashMap<u128, Vec<MessageFragment>>, 
    world: &mut World, 
    resources: &mut Resources
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

            frag_vec.push(fragment.clone());

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
            message_fragments.insert(fragment.uuid, vec![fragment.clone()]);
        }
    }
}

fn client_handle_data(data: DataType, world: &mut World, resources: &mut Resources) {
    match data {
        DataType::ActorToolSelection { client_id, actor_id } => {
            use crate::systems::{
                actor::{
                    Actor, ActorDefinition, Definitions
                },
                selection_box::set_chosen_actor,
            };

            if let Some(definitions) = resources.get::<Definitions<ActorDefinition>>(){
                if let Some(id) = resources.get::<ClientID>() {
                    if id.0 != client_id { //don't act on this client because this was already processed before being sent
                        set_chosen_actor(world, ClientID::new(client_id), &Actor::new(&definitions, actor_id as usize));
                    }
                }

            }
        },
        DataType::ActorChange{ change, store_history } => {
            use crate::systems::{
                actor::{
                    Definitions, 
                    ActorChange,
                    ActorDefinition, 
                    CharacterDefinition, 
                    actor_change, 
                    remove_actor,
                    initialize_actor
                },
                level_map::CoordPos,
            };

            match change {
                ActorChange::ActorInsertion { 
                    uuid: _, 
                    coord_pos: _, 
                    rotation: _, 
                    actor_type: _, 
                    definition_id: _, 
                } => {

                    let actor_definitions = resources.get::<Definitions<ActorDefinition>>().unwrap();

                    actor_change(world, &change, &actor_definitions, None, store_history);

                },
                ActorChange::ActorRemoval(uuid) => {
                    remove_actor(world, uuid);
                }
            }
        },
        DataType::MapInput(r) => {
            if let Some(map) = resources.get::<crate::systems::level_map::Map>().map(|map| *map) {
                map.change(world, r, None);
            }
        },
        DataType::MapChange{ change, store_history } => {

            use crate::systems::{
                level_map,
                level_map::MapChange
            };

            if let Some(map) = resources.get::<crate::systems::level_map::Map>().map(|map| *map) {

                match change {
                    MapChange::MapInsertion { aabb, tile_data } => {
                        map.change(world, level_map::fill_octree_from_aabb(aabb, Some(tile_data)), store_history);
                    },
                    MapChange::MapRemoval(aabb) => {
                        map.change(world, level_map::fill_octree_from_aabb(aabb, None), store_history)
                    },
                    // MapChange::ActorInsertion { uuid, coord_pos, definition_id } => {
                        
                    //     use crate::{
                    //         systems::{
                    //             actor::{ActorDefinitions, initialize_actor},
                    //             level_map::CoordPos,
                    //         }
                    //     };

                    //     //TODO: check if the id exists already, insertion should silently fail if so

                    //     let actors = resources.get::<ActorDefinitions>().unwrap();

                    //     if let Some(actor_definition) = actors.get_definitions().get(definition_id as usize) {
                    //         let entity = initialize_actor(world, actor_definition, CoordPos::new(coord_pos));

                    //         //TODO: add ID component to entity through an entry
                    //     }

                    // },
                    // MapChange::ActorRemoval(uuid) => {
                    //     unimplemented!();
                    // }
                }

            }
        },
        DataType::MapNew => {
            crate::systems::level_map::map_reset(world, resources);
        },
        DataType::HistoryStep{ amount, client_id } => {
            let mut query = <(Write<crate::systems::history::History>, Read<ClientID>)>::query();

            let mut commands = legion::systems::CommandBuffer::new(world);

            if let Some((history, _)) = query.iter_mut(world).filter(|(_, id)| id.val() == client_id).next() {
                history.move_by_step(&mut commands, resources, amount);
            }

            commands.flush(world);
        },
        DataType::UpdateSelectionBounds{client_id: id, coord_pos, aabb} => {

            use crate::systems::selection_box::UpdateBounds;

            //This may seem convoluded, but we only want messages to act on clients that were not the sender,
            // as their update was already handled at the time the message was sent to avoid any perceived input
            // lag on their end. 
            if let Some(client_id) = resources.get::<ClientID>() {
                if id != client_id.0 {

                    world.push((
                        ClientID::new(id),
                        UpdateBounds{
                            coord_pos,
                            aabb
                        }
                    ));

                }
            };

        },
        DataType::CreateSelectionBox{client_id: id, box_type, active, coord_pos, aabb} => {

            use crate::systems::{
                actor::{
                    Actor, Definitions, ActorDefinition,
                },
                selection_box::{
                    ActorToolBox, TerrainToolBox,
                    ToolBoxType, SelectionBox,
                    set_active_selection_box,
                    set_chosen_actor,
                },
                level_map::CoordPos,
                history::History,
            };

            let entity = crate::systems::selection_box::initialize_selection_box(world, id, box_type, None);

            world.push((
                ClientID::new(id),
                History::new() 
             ));
            
            if let Some(mut entry) = world.entry(entity) {
                if let Ok(pos) = entry.get_component_mut::<CoordPos>() {
                    pos.value = coord_pos;
                }
                if let Ok(selection_box) = entry.get_component_mut::<SelectionBox>() {
                    selection_box.aabb = aabb;
                }

                if active {
                    match box_type {
                        ToolBoxType::TerrainToolBox => {
                            set_active_selection_box::<TerrainToolBox>(world, ClientID::new(id));
                        },
                        ToolBoxType::ActorToolBox(actor_id) => {

                            if let Some(actor_definitions) = resources.get::<Definitions<ActorDefinition>>() {
                                set_chosen_actor(world, ClientID(id), &Actor::new(&actor_definitions, actor_id as usize));
                            }

                            set_active_selection_box::<ActorToolBox>(world, ClientID::new(id));
                        }
                    }
                }
                
            }
            
        },
        DataType::ActivateActorToolBox{client_id: id} => {

            use crate::systems::{
                selection_box::{
                    set_active_selection_box,
                    ActorToolBox
                }
            };

            //only set it if it wasn't sent from this client, since it was already handled when the message was sent
            if let Some(client_id) = resources.get::<ClientID>() {
                if client_id.val() != id {
                    set_active_selection_box::<ActorToolBox>(world, ClientID::new(id));
                }
            }
        },
        DataType::ActivateTerrainToolBox{client_id: id} => {

            use crate::systems::{
                selection_box::{
                    set_active_selection_box,
                    TerrainToolBox
                }
            };

            //only set it if it wasn't sent from this client, since it was already handled when the message was sent
            if let Some(client_id) = resources.get::<ClientID>() {
                if client_id.val() != id {
                    set_active_selection_box::<TerrainToolBox>(world, ClientID::new(id));
                }
            }
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