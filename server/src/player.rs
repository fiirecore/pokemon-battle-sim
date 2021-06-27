use std::{collections::VecDeque, sync::Arc};

use common::{
    battle::{
        client::{BattleClient, BattleEndpoint},
        message::{ClientMessage, ServerMessage},
        player::{BattlePlayer, PlayerSettings},
    },
    log::debug,
    net::network::{Endpoint, NetworkController},
    pokedex::pokemon::instance::BorrowedPokemon,
    ser::serialize,
    uuid::Uuid,
    NetServerMessage, Player,
};

use crate::{send, Receiver};

pub struct BattleServerPlayer {
    endpoint: Endpoint,
    controller: Arc<NetworkController>,
    receiver: Arc<Receiver>,
}

impl BattleServerPlayer {
    pub fn player(
        player: (Endpoint, Player),
        controller: Arc<NetworkController>,
        receiver: Arc<Receiver>,
        battle_size: u8,
    ) -> BattlePlayer<Uuid> {
        receiver.insert(player.0, VecDeque::new());
        BattlePlayer::new(
            Uuid::new_v4(),
            player
                .1
                .party
                .into_iter()
                .map(BorrowedPokemon::Owned)
                .collect(),
            Some(player.1.trainer),
            PlayerSettings {
                gains_exp: false,
            },
            Box::new(BattleServerPlayer {
                endpoint: player.0,
                controller,
                receiver,
            }),
            battle_size as usize,
        )
    }
}

impl BattleEndpoint<Uuid> for BattleServerPlayer {
    fn give_client(&mut self, message: ServerMessage<Uuid>) {
        debug!("Endpoint {} is getting sent {:?}", self.endpoint, message);
        send(
            &self.controller,
            self.endpoint,
            &serialize(&NetServerMessage::Game(message)).unwrap(),
        );
    }
}

impl BattleClient<Uuid> for BattleServerPlayer {
    fn give_server(&mut self) -> Option<ClientMessage> {
        self.receiver.get_mut(&self.endpoint).unwrap().pop_front()
    }
}
