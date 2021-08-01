use std::sync::Arc;

use common::{
    battle::{
        message::{ClientMessage, ServerMessage},
        player::{BattlePlayer, PlayerSettings},
        BattleEndpoint,
    },
    net::network::{Endpoint, NetworkController},
    uuid::Uuid,
    NetServerMessage, Player,
};

use log::debug;

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
        receiver.insert(player.0, Default::default());
        BattlePlayer::new(
            Uuid::new_v4(),
            player.1.party,
            Some(player.1.name),
            PlayerSettings {
                gains_exp: false,
                ..Default::default()
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
    fn send(&mut self, message: ServerMessage<Uuid>) {
        debug!("Endpoint {} is getting sent {:?}", self.endpoint, message);
        send(
            &self.controller,
            self.endpoint,
            &crate::serialize(&NetServerMessage::Game(message)),
        );
    }

    fn receive(&mut self) -> Option<ClientMessage> {
        crate::get_endpoint(&self.receiver, &self.endpoint).pop()
    }
}
