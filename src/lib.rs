pub extern crate firecore_battle as battle;

pub use battle::pokedex;
pub extern crate bincode as ser;
pub extern crate message_io as net;
pub extern crate parking_lot as sync;
pub extern crate rand;
pub extern crate uuid;

use battle::message::{ClientMessage, ServerMessage};
use net::network::Transport;
use pokedex::pokemon::PokemonParty;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const DEFAULT_PORT: u16 = 28528;

pub const PROTOCOL: Transport = Transport::FramedTcp;

#[derive(Debug, Deserialize, Serialize)]
pub enum NetClientMessage<'a> {
    Connect(Player, &'a str), // player, dex hashes
    Game(ClientMessage),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum NetServerMessage {
    CanConnect(bool),
    WrongVersion,
    Begin,
    End,
    Game(ServerMessage<Uuid>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Player {
    pub name: String,
    pub party: PokemonParty,
}

#[derive(Debug)]
pub struct Queue<M> {
    inner: VecDeque<M>,
}

impl<M> Default for Queue<M> {
    fn default() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

impl<M> Queue<M> {
    pub fn push(&mut self, message: M) {
        self.inner.push_front(message)
    }

    pub fn pop(&mut self) -> Option<M> {
        self.inner.pop_back()
    }
}
