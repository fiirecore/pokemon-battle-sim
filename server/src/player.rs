use std::{fmt::Debug, sync::Arc};

use message_io::network::{Endpoint, NetworkController};

use common::{
    battle::{
        endpoint::{BattleEndpoint, ReceiveError},
        message::{ClientMessage, ServerMessage},
    },
    NetServerMessage,
};

use serde::Serialize;

use crate::send;

use crossbeam_channel::{Receiver, TryRecvError};

pub struct BattleServerPlayer<ID: Serialize + Debug> {
    endpoint: Endpoint,
    controller: Arc<NetworkController>,
    receiver: Receiver<ClientMessage<ID>>,
}

impl<ID: Serialize + Debug> BattleServerPlayer<ID> {
    pub fn new(
        endpoint: Endpoint,
        controller: &Arc<NetworkController>,
        receiver: Receiver<ClientMessage<ID>>,
    ) -> Box<Self> {

        Box::new(Self {
            endpoint,
            controller: controller.clone(),
            receiver,
        })
    }
}

impl<ID: Serialize + Debug> BattleEndpoint<ID> for BattleServerPlayer<ID> {
    fn send(&mut self, message: ServerMessage<ID>) {
        send(
            &self.controller,
            self.endpoint,
            &crate::serialize(&NetServerMessage::Game(message)),
        );
    }

    fn receive(&mut self) -> Result<ClientMessage<ID>, Option<ReceiveError>> {
        self.receiver.try_recv().map_err(|err| match err {
            TryRecvError::Empty => None,
            TryRecvError::Disconnected => Some(ReceiveError::Disconnected),
        })
    }
}
