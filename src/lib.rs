pub extern crate firecore_battle as battle;
pub extern crate firecore_pokedex as pokedex;
pub extern crate message_io as net;
pub extern crate rand;
pub extern crate uuid;
pub extern crate parking_lot as sync;
pub use firecore_dependencies::*;

use battle::message::{ClientMessage, ServerMessage};
use log::LevelFilter;
use net::network::Transport;
use pokedex::{pokemon::party::PokemonParty, trainer::TrainerData};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const DEFAULT_PORT: u16 = 28528;

pub const PROTOCOL: Transport = Transport::FramedTcp;

#[derive(Debug, Deserialize, Serialize)]
pub enum NetClientMessage {
    Connect(Player),
    Game(ClientMessage),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum NetServerMessage<'a> {
    CanConnect(bool),
    Begin,
    End,
    Game(ServerMessage<'a, Uuid>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Player {
    pub trainer: TrainerData,
    pub party: PokemonParty,
}