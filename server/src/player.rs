use std::{fmt::Debug, sync::Arc};

use common::{
    battle::{
        endpoint::{BattleEndpoint, ReceiveError},
        message::{ClientMessage, ServerMessage},
    },
    net::network::{Endpoint, NetworkController},
    NetServerMessage,
};

use log::debug;
use serde::Serialize;

use crate::{send, Receiver};

pub struct BattleServerPlayer<ID: Serialize + Debug> {
    endpoint: Endpoint,
    controller: Arc<NetworkController>,
    receiver: Arc<Receiver<ID>>,
}

impl<ID: Serialize + Debug> BattleServerPlayer<ID> {
    pub fn new(
        endpoint: Endpoint,
        controller: &Arc<NetworkController>,
        receiver: &Arc<Receiver<ID>>,
    ) -> Box<Self> {
        receiver.insert(endpoint, Default::default());

        Box::new(Self {
            endpoint,
            controller: controller.clone(),
            receiver: receiver.clone(),
        })
    }
}

impl<ID: Serialize + Debug, const AS: usize> BattleEndpoint<ID, AS> for BattleServerPlayer<ID> {
    fn send(&mut self, message: ServerMessage<ID, AS>) {
        debug!("Endpoint {} is getting sent {:?}", self.endpoint, message);
        send(
            &self.controller,
            self.endpoint,
            &crate::serialize(&NetServerMessage::Game(message)),
        );
    }

    fn receive(&mut self) -> Result<ClientMessage<ID>, Option<ReceiveError>> {
        crate::get_endpoint(&self.receiver, &self.endpoint)
            .pop()
            .ok_or(None)
    }
}
