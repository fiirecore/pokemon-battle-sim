pub extern crate firecore_game as game;
pub extern crate simple_logger as logger;
pub extern crate laminar;

pub static DEX_BYTES: &[u8] = include_bytes!("../dex.bin");

pub const SERVER_PORT: u16 = 14191;

use std::net::SocketAddr;
use game::pokedex::{
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

pub fn init() {
    simple_logger::SimpleLogger::new().init().unwrap();
    pokedex_no_ctx(game::deps::ser::deserialize(DEX_BYTES).unwrap());
}

pub fn pokedex_no_ctx(dex: game::pokedex::serialize::SerializedDex) {

    use game::pokedex::Dex;

    game::pokedex::pokemon::Pokedex::set(dex.pokemon.into_iter().map(|p| (p.pokemon.id, p.pokemon)).collect());

    game::pokedex::moves::Movedex::set(dex.moves.into_iter().map(|m| (m.pokemon_move.id, m.pokemon_move)).collect());

    game::pokedex::item::Itemdex::set(dex.items.into_iter().map(|i| (i.item.id, i.item)).collect());

}

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