pub extern crate firecore_pokedex as pokedex;
pub extern crate simple_logger as logger;
pub extern crate laminar;

pub const SERVER_PORT: u16 = 14191;

use std::net::SocketAddr;
use pokedex::{
    pokemon::party::PokemonParty,
    trainer::{TrainerId, TrainerData},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub enum NetClientMessage {
    RequestConnect,
    Connect(Player)
}

#[derive(Deserialize, Serialize)]
pub enum NetServerMessage {
    AcceptConnect,
    // Begin,
}

#[derive(Deserialize, Serialize)]
pub struct Player {
    pub id: TrainerId,
    pub trainer: TrainerData,
    pub party: PokemonParty,
    pub client: NetBattleClient,
}

#[derive(Deserialize, Serialize)]
pub struct NetBattleClient(pub SocketAddr);

use std::net::{UdpSocket, IpAddr};

/// get the local ip address, return an `Option<String>`. when it fail, return `None`.
pub fn ip() -> Option<IpAddr> {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return None,
    };

    match socket.connect("8.8.8.8:80") {
        Ok(()) => (),
        Err(_) => return None,
    };

    socket.local_addr().ok().map(|addr| addr.ip())
}