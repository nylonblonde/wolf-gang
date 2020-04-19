use legion::prelude::*;

use std::net::SocketAddr;

use laminar::{Packet, Socket, SocketEvent};

use serde_derive::{Deserialize, Serialize};

type Point = nalgebra::Vector3<i32>;

pub struct ServerSocket {
    socket: Socket
}

impl ServerSocket {
    pub fn new<T: ToString>(address: T) -> Self {
        let addr: SocketAddr = address.to_string().parse().unwrap();
        Self {
            socket: Socket::bind(addr).unwrap()
        }
    }
}

pub struct ClientSocket {
    socket: Socket
}

impl ClientSocket {
    pub fn new<T: ToString>(address: T) -> Self {
        let addr: SocketAddr = address.to_string().parse().unwrap();
        Self {
            socket: Socket::bind(addr).unwrap()
        }
    }
}

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
            while let Some(pkt) = server.socket.recv() {
                match pkt {
                    SocketEvent::Packet(pkt) => {

                        //write to or insert message components that have this data type?

                    },
                    _ => {}
                }
            }

        })
}

