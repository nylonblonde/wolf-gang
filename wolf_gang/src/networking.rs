use crate::{
    game_state::{GameState, GameStateTraits, NewState},
    systems::{
        networking::{
            ClientID,
            Disconnection,
            MessageSender,
            ServerMessageSender,
        }
    }
};
use legion::*;

use cobalt::{
    BinaryRateLimiter, 
    Client,
    Config, 
    NoopPacketModifier, 
    Server, 
    Socket,
};

use std::{
    io::{Error, ErrorKind},
    net,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs},
    sync::{
        mpsc::TryRecvError,
    },
};

type Point = nalgebra::Vector3<i32>;

pub const MULTICAST_ADDR_V4: &str = "234.2.2.2:12345";
pub const LOOPBACK_ADDR_V4: &str = "127.0.0.1:12345";
// const LOBBY_ADDR_V4: &'static str = "";

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Scope{
    Online(SocketAddr),
    Multicast,
    Loopback
}

#[derive(Debug, Copy, Clone)]
pub struct Connection {
    conn_type: ConnectionType,
    scope: Scope,
    state: ConnectionState
}

impl Connection {
    pub fn new(conn_type: ConnectionType, scope: Scope) -> Connection {
        Connection {
            conn_type,
            scope,
            ..Default::default()
        }
    }

    pub fn get_type(&self) -> ConnectionType {
        self.conn_type
    }

    pub fn get_scope(&self) -> Scope {
        self.scope
    }
}

impl Default for Connection {
    fn default() -> Connection {
        Connection {
            conn_type: ConnectionType::Host,
            scope: Scope::Loopback,
            state: ConnectionState::NotConnected
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ConnectionType {
    Join,
    Host,
}

#[derive(Debug, Copy, Clone)]
pub enum ConnectionState {
    NotConnected,
    AttemptingConnection,
    Connected
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
    pub fn connect_multicast(&mut self, addr: SocketAddr) -> Result<(), Error> {
        match self.multicast_addr {
            None => {
                self.multicast_addr = Some(addr);
                Ok(())
            },
            Some(_) => {
                Err(Error::new(ErrorKind::AlreadyExists, ""))
            }
        }
    }

}

// Modified from the cobalt source Copyright (c) 2015-2017 Ivo Wetzel
// Redefined just because I needed more access to the net::UdpSocket
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
            socket,
            buffer,
            multicast_addr: None
        })

    }

    /// Attempts to return a incoming packet on this socket without blocking.
    fn try_recv(&mut self) -> Result<(net::SocketAddr, Vec<u8>), TryRecvError> {

        if let Ok((len, src)) = self.socket.recv_from(&mut self.buffer) {

            //Ip requests are sent as 8008
            if self.buffer[0..4] == [8,0,0,8] {
                // println!("Got the IP request from {:?}", src);
                // println!("{:?}", self.socket);
                self.send_to(&[], src).unwrap();
            }
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
                // println!("Sending via multicast");
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
}

impl GameStateTraits for Networking {

    fn initialize(&mut self, world: &mut World, resources: &mut Resources) {
        
        let connection = *resources.get_or_default::<Connection>();

        let mut config = Config{
            packet_drop_threshold: std::time::Duration::from_secs(30),
            connection_drop_threshold: std::time::Duration::from_secs(15),
            connection_init_threshold: std::time::Duration::from_secs(15),
            ..Default::default()
        };

        if let Scope::Loopback = connection.scope {
            config.packet_max_size = 6000;
        }

        resources.insert(ClientID::default());

        if let ConnectionType::Host = connection.conn_type {
            let entity = world.push(
                (
                    Server::<UdpSocket, BinaryRateLimiter, NoopPacketModifier>::new(config),
                )
            );

            if let Some(entry) = world.entry(entity) {
                if let Ok(server) = entry.into_component_mut::<Server<UdpSocket, BinaryRateLimiter, NoopPacketModifier>>() {
                    let server_addr = match connection.scope {
                        Scope::Loopback => SocketAddr::from(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 12345)),
                        Scope::Multicast => SocketAddr::from(SocketAddrV4::new(Ipv4Addr::new(0,0,0,0), 12345)),
                        Scope::Online(_) => SocketAddr::from(SocketAddrV4::new(Ipv4Addr::new(0,0,0,0), 3450))
                    //     Scope::Online => {
        
                    //         lobby_client = Some(Client::new(Config{
                    //             send_rate: 1,
                    //             connection_init_threshold: Duration::from_secs(3),
                    //             ..Default::default()
                    //         }));
                    //         let lobby_client = lobby_client.as_mut().unwrap();
                    //         lobby_client.connect(LOBBY_ADDR_V4).unwrap();
                            
                    //         'lobby: loop {
        
                    //             //Receive IP Address once we've been registered as a host
                    //             while let Ok(event) = lobby_client.receive() {
                    //                 match event {
                    //                     ClientEvent::Message(message) => {
                    //                         let data: lobby::DataType = bincode::deserialize(&message).unwrap();
        
                    //                         match data {
                    //                             lobby::DataType::Host(host) => {
                    //                                 let addr = host.get_addr();
                    //                                 //unwrap is okay because we know we'd have this set above
                    //                                 host_addr_tx.unwrap().send(addr).unwrap();
                    //                                 break 'lobby addr
                    //                             },
                    //                             _ => {}
                    //                         }
                    //                     },
                    //                     _ => {}
                    //                 }
                    //             }
                                
                    //             if let Ok(conn) = lobby_client.connection() {
                    //                 conn.send(MessageKind::Instant, 
                    //                     bincode::serialize(
                    //                         &lobby::DataType::RequestHost(lobby::Config::default())
                    //                     ).unwrap()
                    //                 )
                    //             }
        
                    //             if let Ok(_) = lobby_client.send(true) {};
                    //         }
                    //     }
                    };

                    // Server binding silently fails if address is in use as a way of allowing multiple clients on the same machine
                    if server.listen(server_addr).is_ok() { 
                        println!("Server is listening at {}", server_addr);

                        if let Scope::Multicast = connection.scope {
                            server.socket().unwrap().join_multicast(&MULTICAST_ADDR_V4.parse::<SocketAddr>().unwrap().ip(), &IpAddr::V4(Ipv4Addr::new(0,0,0,0)))
                        } 
                    }
                }
            }
        }

        let entity = world.push(
            (
                Client::<UdpSocket, BinaryRateLimiter, NoopPacketModifier>::new(config),
            )
        );

        if let Some(entry) = world.entry(entity) {

            if let Ok(client) = entry.into_component_mut::<Client<UdpSocket, BinaryRateLimiter, NoopPacketModifier>>() {

                let client_addr: Option<SocketAddr> = match connection.scope {
                    Scope::Loopback => Some(LOOPBACK_ADDR_V4.parse::<SocketAddr>().unwrap()),
                    Scope::Multicast => {

                        //return None and we'll offload connection logic to systems so that we can retrieve the IP address
                        None
                        
                    },
                    Scope::Online(host) => Some(host)
                    // Scope::Online(host) => {
                    //     match connection.conn_type {
                    //         ConnectionType::Host => {
                    //             let host_option = host_addr_rx.unwrap();
                    //             let host_addr_lock = host_option.lock().unwrap();

                    //             if let Ok(addr) = host_addr_lock.recv() {
                    //                 addr
                    //             } else {
                    //                 panic!("Couldn't receive")
                    //             }
                    //         },
                    //         ConnectionType::Join => {
                    //             todo!("Wait for a suitable host to be chosen")
                    //         }
                    //     }
                    // }
                };

                if let Some(client_addr) = client_addr {
                    match connection.scope {
                        Scope::Loopback => {
                            client.connect(client_addr).expect("Couldn't connect to client address!");
                            println!("[Client] {:?} Connecting to {:?}...", client.socket().unwrap().local_addr().unwrap(), client.peer_addr().unwrap());
                        },
                        Scope::Online(_) => {
                            client.connect(client_addr).expect("Couldn't connect to online address!");
                            println!("[Client] {:?} Connecting to {:?}...", client.socket().unwrap().local_addr().unwrap(), client.peer_addr().unwrap());
                        },
                        _ => {}
                    }
                }
            }
        }  

        //In the case of Multicast's scope, we wouldn't have connected yet because we need to get the host's IP Address. Offload IP
        // address retrieval to systems
        if let Scope::Multicast = connection.scope {
            if let Some(mut entry) = world.entry(entity) {
                let socket = net::UdpSocket::bind("0.0.0.0:0").unwrap();
                socket.set_nonblocking(true).ok();
                entry.add_component(socket);
            }
        }
    }

    fn free(&mut self, world: &mut World, resources: &mut Resources) {
        
        let mut query = <Read<ClientID>>::query();
        let disconnections = query.iter(world)
            .map(|client_id| (Disconnection::new(client_id.val()),))
            .collect::<Vec<(Disconnection,)>>();

        //queue up on_disconnection for every object that has a ClientID
        world.extend(disconnections);

        resources.insert(ClientID::new(0));

        //get rid of any message senders that might still exist
        let mut query = <(Entity, Read<MessageSender>)>::query();
        let mut entities = query.iter(world).map(|(entity, _)| *entity).collect::<Vec<Entity>>();

        let mut query = <(Entity, Read<ServerMessageSender>)>::query();
        entities.extend(query.iter(world).map(|(entity, _)| *entity).collect::<Vec<Entity>>());

        let mut query = <(Entity, Write<Client<UdpSocket, BinaryRateLimiter, NoopPacketModifier>>)>::query();
        query.for_each_mut(world, |(entity, client)| {
            client.disconnect().ok();

            entities.push(*entity);
        });

        let mut query = <(Entity, Write<Server<UdpSocket, BinaryRateLimiter, NoopPacketModifier>>)>::query();

        query.for_each_mut(world, |(entity, server)| {
            server.shutdown().ok();

            entities.push(*entity);
        });

        entities.into_iter().for_each(|entity| {
            world.remove(entity);
        });

        // Not removing the Connection resource so we can do a reset without having to store and restate Connection.
        // Would be overwritten in the case of a new type of connection anyway
        // resources.remove::<Connection>();

    }

}

impl NewState for Networking {

    fn new(name: &'static str, active: bool) -> Self {

        Self {
            game_state: GameState::new(
                name,
                active,
            ),
            
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

/// Get local ip based on connection to the connect value IP
fn get_local_ip<T: ToSocketAddrs>(connect: T) -> Option<SocketAddr> {
    match net::UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => {
            match socket.connect(connect) {
                Err(_) => {
                    None
                },
                Ok(_) => {
                    match socket.local_addr() {
                        Ok(local_addr) => Some(local_addr),
                        Err(_) => None,
                    }
                }
            }
        },
        Err(_) => None,
    }
}


