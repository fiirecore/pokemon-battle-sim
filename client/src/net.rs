use std::net::SocketAddr;

use naia_client_socket::{Packet, PacketReceiver as NaiaPacketReceiver, PacketSender as NaiaPacketSender};

pub type Endpoint = SocketAddr;

pub struct PacketSender(NaiaPacketSender);

impl PacketSender {

    pub fn send(&mut self, bytes: Vec<u8>) {
        self.0.send(Packet::new(bytes))
    }

}

impl From<NaiaPacketSender> for PacketSender {
    fn from(p: NaiaPacketSender) -> Self {
        Self(p)
    }
}