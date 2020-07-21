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
    Socket,
    // UdpSocket,
};

use std::{
    io::{Error, ErrorKind},
    net,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        mpsc,
        mpsc::TryRecvError,
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering} 
    }
};

use snap::raw::{Decoder, Encoder};

use bincode::{deserialize, serialize};

use std::rc::Rc;
use std::time::{Duration, Instant};

// static KILL_NETWORK_THREAD: AtomicBool = AtomicBool::new(false); 

pub struct ClientAddr(pub SocketAddr);
pub struct ServerAddr(pub SocketAddr);

impl Default for ClientAddr {
    fn default() -> Self {
        Self("0.0.0.0:12345".parse().unwrap())
    }
}

impl Default for ServerAddr {
    fn default() -> Self {
        Self("0.0.0.0:12345".parse().unwrap())
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

#[derive(Debug)]
pub struct UdpSocket {
    socket: std::net::UdpSocket,
    buffer: Vec<u8>,
    multicast_addr: Option<SocketAddr>
}

impl UdpSocket {

    /// Literally just self.socket.broadcast()
    fn can_broadcast(&self) -> bool {
        self.socket.broadcast().unwrap()
    }

    /// Literally just self.socket.set_broadcast
    fn set_broadcast(&mut self, can_broadcast: bool) {
        self.socket.set_broadcast(can_broadcast).unwrap();
    }

    fn join_multicast(&mut self, addr: &IpAddr, interface: &IpAddr) {
        match (addr, interface) {
            (IpAddr::V4(addr), IpAddr::V4(interface)) => {
                self.socket.join_multicast_v4(addr, interface).unwrap();
            },
            _ => {
                todo!("Support for ipv6")
            }
        }
    }

    /// Connects clients to multicast for sending
    fn connect_multicast(&mut self, addr: &SocketAddr) -> Result<(), Error> {
        match self.multicast_addr {
            None => {
                self.multicast_addr = Some(*addr);
                Ok(())
            },
            Some(_) => {
                Err(Error::new(ErrorKind::AlreadyExists, ""))
            }
        }
    }

}

// From the cobalt source Copyright (c) 2015-2017 Ivo Wetzel
// Redefined just because I more access to the net::UdpSocket
impl Socket for UdpSocket {
    /// Tries to create a new UDP socket by binding to the specified address.
    fn new<T: net::ToSocketAddrs>(address: T, max_packet_size: usize) -> Result<Self, Error> {

        // Create the send socket
        let socket = net::UdpSocket::bind(address)?;

        // Switch into non-blocking mode
        socket.set_nonblocking(true)?;

        // Allocate receival buffer
        let buffer: Vec<u8> = std::iter::repeat(0).take(max_packet_size).collect();

        Ok(UdpSocket {
            socket: socket,
            buffer: buffer,
            multicast_addr: None
        })

    }

    /// Attempts to return a incoming packet on this socket without blocking.
    fn try_recv(&mut self) -> Result<(net::SocketAddr, Vec<u8>), TryRecvError> {

        if let Ok((len, src)) = self.socket.recv_from(&mut self.buffer) {
            Ok((src, self.buffer[..len].to_vec()))

        } else {
            Err(TryRecvError::Empty)
        }
    }

    /// Send data on the socket to the given address. On success, returns the
    /// number of bytes written.
    fn send_to(&mut self, data: &[u8], addr: net::SocketAddr) -> Result<usize, Error> {
        match self.multicast_addr {
            Some(multicast_addr) => {
                println!("Sending via multicast");
                self.socket.send_to(data, multicast_addr)
            },
            None => self.socket.send_to(data, addr)
        }
    }

    /// Returns the socket address of the underlying `net::UdpSocket`.
    fn local_addr(&self) -> Result<net::SocketAddr, Error> {
        self.socket.local_addr()
    }
}

pub struct Networking {
    game_state: GameState,
    server_quit_tx: mpsc::Sender<()>,
    server_quit_rx: Arc<Mutex<mpsc::Receiver<()>>>,
    client_quit_tx: mpsc::Sender<()>,
    client_quit_rx: Arc<Mutex<mpsc::Receiver<()>>>,
    client_running: Arc<(Mutex<bool>, Condvar)>,
    server_running: Arc<(Mutex<bool>, Condvar)>
}

impl GameStateTraits for Networking {

    fn initialize(&mut self, _: &mut World, resources: &mut Resources) {
        resources.insert(MessagePool{
            messages: Vec::new()
        });

        let server_addr = resources.get_mut_or_default::<ServerAddr>().unwrap().0;

        let mut config = Config{
            packet_drop_threshold: std::time::Duration::from_secs(30),
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

        //Set up the server
        let mut server = Server::<UdpSocket, BinaryRateLimiter, NoopPacketModifier>::new(config.clone());
        server.listen(server_addr).expect("Failed to bind to socket.");
        server.socket().unwrap().join_multicast(&IpAddr::V4(Ipv4Addr::new(234,2,2,2)), &IpAddr::V4(Ipv4Addr::new(0,0,0,0)));
        println!("[Server] Listening at {:?}...", server.socket().unwrap().local_addr().unwrap());

        let multicast_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(234,2,2,2)), 12345);

        // let server_addr = match net::UdpSocket::bind("0.0.0.0:0") {
        //     Ok(socket) => {
        //         if let Err(_) = socket.connect(multicast_addr) {
        //             None
        //         } else {
        //             match socket.peer_addr() {
        //                 Ok(local_addr) => Some(local_addr),
        //                 Err(_) => None,
        //             }
        //         }
        //     },
        //     Err(_) => None,
        // };

        // println!("{:?}", server_addr);

        //Set up the client
        let mut client: Client<UdpSocket, BinaryRateLimiter, NoopPacketModifier> = Client::new(config);

        //Client can only connect succesfully if we connect to the specific host IP address, not multicast. Not ideal
        client.connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 25)), 12345)).expect("Couldn't connect to global address!");
        client.socket().unwrap().connect_multicast(&multicast_addr).unwrap();        

        println!("[Client] {:?} Connecting to {:?}...", client.socket().unwrap().local_addr().unwrap(), client.peer_addr().unwrap());

        let quitter = self.server_quit_rx.clone();
        let running_pair = self.server_running.clone();

        std::thread::spawn(move || {
            let mut encoder = Encoder::new();
            let mut decoder = Decoder::new();

            let (lock, cvar) = &*running_pair;
            let mut running = lock.lock().unwrap();

            *running = true;

            'server: loop {

                let quit_receiver = &*quitter.lock().unwrap(); 
                match quit_receiver.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        println!("[Server] Thread discontinued");
                        break 'server;
                    },
                    _ => {}
                }

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

            *running = false;
            cvar.notify_one();

            server.shutdown().unwrap();
        });

        let quitter = self.client_quit_rx.clone();
        let running_pair = self.client_running.clone();

        std::thread::spawn(move ||{

            let mut message_fragments: HashMap::<u128, Vec<MessageFragment>> = HashMap::new();
            let mut encoder = Encoder::new();
            let mut decoder = Rc::new(Decoder::new());

            let (lock, cvar) = &*running_pair;
            let mut running = lock.lock().unwrap();

            *running = true;

            'client: loop {

                let quit_receiver = &*quitter.lock().unwrap();
                match quit_receiver.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        println!("[Client] Thread discontinued");
                        break 'client;
                    },
                    _ => {}
                }

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
                    // println!("{:?} {:?}", conn.peer_addr(), conn.local_addr());

                    let mut game_lock = crate::GAME_UNIVERSE.lock().unwrap();
                    let game = &mut *game_lock;
                    let resources = &mut game.resources;

                    match resources.get_mut::<MessagePool>() {
                        Some(mut message_pool) => {
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
                        },
                        None => break 'client
                    };
                    
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

            *running = false;
            cvar.notify_one();

            client.disconnect().ok();
        });
    }

    fn free(&mut self, world: &mut World, resources: &mut Resources) {
        
        //If a thread is running for the client, send a signal to kill it
        self.client_quit_tx.send(()).unwrap();

        //If a thread is running for the server, send a signal to kill it
        self.server_quit_tx.send(()).unwrap();

        //Grab our Convar pairs for the server and client, and wait until they tell us that their respective threads are done
        let (client_lock, client_cvar) = &*self.client_running;
        let (server_lock, server_cvar) = &*self.server_running;

        let mut client_running = client_lock.lock().unwrap();
        let mut server_running = server_lock.lock().unwrap();

        while *client_running && *server_running {
            client_running = client_cvar.wait(client_running).unwrap();
            server_running = server_cvar.wait(server_running).unwrap();
        }

        //reset the messagepool -- can't just delete it because of the legion system grabbing it
        resources.insert(MessagePool{
            messages: Vec::new()
        });

        //get rid of any message senders that might still exist
        let query = <Read<MessageSender>>::query();
        let entities = query.iter_entities(world).map(|(entity, _)| entity).collect::<Vec<Entity>>();

        for entity in entities {
            world.delete(entity);
        }

        //Removes the ClientAddr resource which would have been set if we were connected to a non-local host
        resources.remove::<ClientAddr>();
        //Removes the ServerAddr resource which would have been set if we were hosting
        resources.remove::<ServerAddr>();

    }

}

impl NewState for Networking {

    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {

        let (server_quit_tx, server_quit_rx) = mpsc::channel::<()>();
        let (client_quit_tx, client_quit_rx) = mpsc::channel::<()>();

        Self {
            game_state: GameState::new(
                name,
                schedule,
                active,
            ),
            server_quit_tx: server_quit_tx,
            server_quit_rx: Arc::new(Mutex::new(server_quit_rx)),

            client_quit_tx: client_quit_tx,
            client_quit_rx: Arc::new(Mutex::new(client_quit_rx)),

            client_running: Arc::new((Mutex::new(false), Condvar::new())),
            server_running: Arc::new((Mutex::new(false), Condvar::new())),
            
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
