use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use naia_server_socket::{
    Packet, PacketReceiver as NaiaPacketReceiver, PacketSender as NaiaPacketSender, ServerAddrs,
    Socket as NaiaSocket,
};

pub type Endpoint = SocketAddr;

pub struct Socket(NaiaSocket);

impl Socket {
    pub fn new(port: u16) -> Self {
        let local = IpAddr::V4(Ipv4Addr::LOCALHOST);

        let address = SocketAddr::new(local, port);

        let webrtc = SocketAddr::new(local, port + 1);

        let server_addresses = ServerAddrs::new(address, webrtc, webrtc);

        let mut socket = NaiaSocket::new(Default::default()); // SocketConfig::new(LinkConditionerConfig::))

        socket.listen(server_addresses);

        Socket(socket)
    }

    pub fn sender(&self) -> PacketSender {
        self.0.get_packet_sender().into()
    }

    pub fn receiver(&self) -> PacketReceiver {
        self.0.get_packet_receiver().into()
    }
}

#[derive(Clone)]
pub struct PacketSender(NaiaPacketSender);

impl PacketSender {
    pub fn send(&self, endpoint: Endpoint, bytes: Vec<u8>) {
        self.0.send(Packet::new(endpoint, bytes))
    }
}

impl From<NaiaPacketSender> for PacketSender {
    fn from(s: NaiaPacketSender) -> Self {
        Self(s)
    }
}

pub struct PacketReceiver(NaiaPacketReceiver);

impl PacketReceiver {
    pub fn receive(&mut self) -> Option<Packet> {
        match self.0.receive() {
            Ok(packet) => packet,
            Err(err) => {
                log::error!("Cannot receive packets with error {}", err);
                None
            }
        }
    }
}

impl From<NaiaPacketReceiver> for PacketReceiver {
    fn from(r: NaiaPacketReceiver) -> Self {
        Self(r)
    }
}
// pub fn listen(port: u16) -> (PacketSender, PacketReceiver) {

// }
