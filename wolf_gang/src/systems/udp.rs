use legion::prelude::*;

use std::net::{IpAddr, SocketAddr};

use laminar::{Packet, Socket, SocketEvent};

use serde_derive::{Deserialize, Serialize};

type Point = nalgebra::Vector3<i32>;

pub struct ServerSocket(pub BaseSocket);

pub struct ServerAddr(pub SocketAddr);

pub struct ClientSocket(pub BaseSocket);

pub struct BaseSocket(pub Socket);

impl BaseSocket {
    pub fn new(address: &'static str) -> Self {
        Self(Socket::bind(address.parse::<SocketAddr>().unwrap()).unwrap())
    }
}

// When player is hosting, their server socket connection gets overwritten with this address 
pub const LAN_SERVER_ADDR: &'static str = "255.255.255.255:5432";

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataType {
    Point {
        point: Point
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Message {
    pub data_type: DataType
}

pub fn create_message_receiving_system() -> Box<dyn Schedulable> {

    SystemBuilder::new("message_receiving_system")
        .write_resource::<ServerSocket>()
        // .with_query(<(Write<Message>, Tagged<DataType>)>::query())
        .build(move |commands, world, server, query| {
            
            //receive packets
            while let Some(pkt) = (server.0).0.recv() {
                match pkt {
                    SocketEvent::Packet(pkt) => {

                        //write to or insert message components that have this data type?

                    },
                    _ => {}
                }
            }

        })
}

