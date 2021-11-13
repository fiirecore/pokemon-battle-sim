use std::fmt::Debug;

// use message_io::network::{Endpoint, NetworkController};

use common::{
    battle::{
        endpoint::{BattleEndpoint, ReceiveError},
        message::{ClientMessage, ServerMessage},
    },
    NetServerMessage,
};

use firecore_battle_net::pokedex::pokemon::{owned::SavedPokemon, party::Party, stat::StatSet};
use rand::Rng;
use serde::Serialize;

use crate::net::*;

use crossbeam_channel::{Receiver, TryRecvError};

pub struct BattleServerPlayer<ID: Serialize + Debug> {
    endpoint: Endpoint,
    sender: PacketSender,
    receiver: Receiver<ClientMessage<ID>>,
}

impl<ID: Serialize + Debug> BattleServerPlayer<ID> {
    pub fn new(
        endpoint: Endpoint,
        sender: &PacketSender,
        receiver: Receiver<ClientMessage<ID>>,
    ) -> Box<Self> {
        Box::new(Self {
            endpoint,
            sender: sender.clone(),
            receiver,
        })
    }
}

impl<ID: Serialize + Debug> BattleEndpoint<ID> for BattleServerPlayer<ID> {
    fn send(&mut self, message: ServerMessage<ID>) {
        self.sender.send(
            self.endpoint,
            crate::serialize(&NetServerMessage::Game(message)),
        );
    }

    fn receive(&mut self) -> Result<ClientMessage<ID>, Option<ReceiveError>> {
        self.receiver.try_recv().map_err(|err| match err {
            TryRecvError::Empty => None,
            TryRecvError::Disconnected => Some(ReceiveError::Disconnected),
        })
    }
}

pub fn generate_party(random: &mut impl Rng, pokedex_len: u16) -> Party<SavedPokemon> {
    let mut party = Party::new();

    for _ in 0..party.capacity() {
        let id = random.gen_range(1..pokedex_len);
        party.push(SavedPokemon::generate(
            random,
            id,
            50,
            None,
            Some(StatSet::uniform(15)),
        ));
    }

    party
}