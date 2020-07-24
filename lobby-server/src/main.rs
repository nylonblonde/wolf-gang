use std::{
    collections::VecDeque,
    net,
    net::{
        IpAddr, Ipv4Addr, SocketAddr,
    },
    thread::sleep,
    time::{Instant, Duration},
};

use bincode;

use cobalt::{
    Config, UdpSocket, BinaryRateLimiter, NoopPacketModifier, Server, ServerEvent, MessageKind
};

use lobby::{
    DataType,
    Host
};

fn main() {

    let mut hosts: VecDeque<Host> = VecDeque::new();
    
    let mut server: Server<UdpSocket, BinaryRateLimiter, NoopPacketModifier> = Server::new(Config::default());
    server.listen("0.0.0.0:3450").unwrap(); //this is the only unwrap that should be allowed in this file

    loop {

        while let Ok(event) = server.accept_receive() {
            match event {
                ServerEvent::Connection(id) => {
                    println!("Connection established {:?}", id);
                },
                ServerEvent::ConnectionClosed(id, _) | ServerEvent::ConnectionLost(id, _) => {
                    println!("Connection to {:?} lost", id);
                },
                ServerEvent::Message(id, message) => {
                    match bincode::deserialize(&message) {
                        Ok(data) => {
                            println!("Received {:?} from conn id: {:?}", data, id);

                            match data {
                                DataType::RequestHost(config) => {

                                    match server.connections().get_mut(&id) {
                                        Some(connection) => {
                                            let host = Host::new(config, connection.peer_addr());

                                            match bincode::serialize(&DataType::Host(host)) {
                                                Ok(payload) => {
                                                    connection.send(MessageKind::Reliable, payload)
                                                },
                                                Err(err) => println!("{:?}, Failed to serialize, no message sent to {:?}", err, id)
                                            }
                                        },
                                        None => println!("id {:?} was not found in connections", id)
                                    }
                                },
                                DataType::RequestJoin(host) => {

                                },
                                _ => {}
                            }
                        },
                        Err(err) => println!("{:?} Couldn't deserialize message from conn id: {:?} {:?}", err, id, message)
                    }
                },
                _ => {}
            }
        }

        if let Ok(_) = server.send(true){}

    }
    
}
