pub extern crate firecore_battle as battle;

pub use battle::pokedex;
pub use bincode::{deserialize, serialize, Error as SerdeError};
pub extern crate bincode;
pub extern crate rand;

use battle::{
    message::{ClientMessage, ServerMessage},
    pokedex::pokemon::{owned::SavedPokemon, party::Party},
};
use serde::{Deserialize, Serialize};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub type Id = u8;

pub const DEFAULT_PORT: u16 = 28528;
// pub const PROTOCOL: Transport = Transport::FramedTcp;

#[derive(Debug, Deserialize, Serialize)]
pub enum NetClientMessage<ID> {
    /// Request to connect with version string
    RequestJoin(String),
    /// Join the server
    Join(Player),
    /// Send game messages to server
    Game(ClientMessage<ID>),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum NetServerMessage<ID> {
    Validate(ConnectMessage),
    Game(ServerMessage<ID>),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ConnectMessage {
    CanJoin,
    /// Client has not requested to join by sending version
    NoRequest,
    AlreadyConnected,
    ConnectionReplaced,
    WrongVersion,
    InProgress,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Player {
    pub name: String,
    pub party: Party<SavedPokemon>,
}