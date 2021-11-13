use gui::pokedex::engine::log::{debug, error, info, warn};
use naia_client_socket::{PacketReceiver, Socket};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, hash::Hash};

use common::{
    battle::{
        endpoint::{BattleEndpoint, MpscEndpoint},
        message::ServerMessage,
    },
    deserialize,
    pokedex::{item::Item, moves::Move, pokemon::Pokemon},
    serialize, ConnectMessage, NetClientMessage, NetServerMessage, Player, VERSION,
};

use gui::BattlePlayerGui;

use crate::{ConnectState, GameContext, GuiPlayer, net::{Endpoint, PacketSender}};

pub struct BattleConnection {
    socket: Socket,
    // endpoint: Endpoint,
    sender: PacketSender,
    receiver: PacketReceiver,
    // receiver: Receiver<NetServerMessage<ID>>,
    name: Option<String>,
    accumulator: f32,
}

impl BattleConnection {
    pub fn connect(address: Endpoint, name: Option<String>) -> Option<Self> {

        let mut socket = Socket::new(Default::default());

        socket.connect(address);

        let sender = PacketSender::from(socket.get_packet_sender());
        let receiver = socket.get_packet_receiver();

        // let mut socket = QuadSocket::connect(address).ok()?;

        Some(Self {
            socket,
            sender,
            receiver,
            // endpoint: address,
            name,
            accumulator: 9.9,
        })
    }

    pub fn end<ID: Serialize>(&mut self) {
        self.sender
            .send(serialize(&NetClientMessage::<ID>::Leave).unwrap());
        // self.controller.remove(self.endpoint.resource_id());
    }

    pub(crate) fn wait_confirm<ID: Serialize + DeserializeOwned>(
        &mut self,
        ctx: &mut GameContext,
        player: &mut GuiPlayer,
        delta: f32,
    ) -> Option<ConnectState> {
        self.accumulator += delta;
        if self.accumulator >= 10.0 {
            self.sender.send(
                serialize(&NetClientMessage::<ID>::RequestJoin(VERSION.to_owned())).unwrap(),
            );
            self.accumulator -= 10.0;
        }
        if let Some(message) = self.recv::<ID>() {
            match message {
                NetServerMessage::Validate(message) => {
                    return Some(match message {
                        ConnectMessage::CanJoin(party) => {
                            info!("Server accepted connection!");

                            let name = self.name.take().unwrap_or_else(|| {
                                use rand::{distributions::Alphanumeric, Rng};
                                std::iter::repeat(())
                                    .map(|()| ctx.random.sample(Alphanumeric))
                                    .map(char::from)
                                    .take(7)
                                    .collect()
                            });

                            let m = NetClientMessage::<ID>::Join(Player { name });

                            self.send(&m);

                            if let NetClientMessage::Join(p) = m {
                                let pokedex = unsafe { crate::POKEDEX.as_ref().unwrap() };
                                let movedex = unsafe { crate::MOVEDEX.as_ref().unwrap() };
                                let itemdex = unsafe { crate::ITEMDEX.as_ref().unwrap() };
                                player.party = party
                                    .into_iter()
                                    .map(|o| {
                                        o.init(&mut ctx.random, pokedex, movedex, itemdex)
                                            .unwrap_or_else(|| {
                                                panic!("Could not initialize generated pokemon!")
                                            })
                                    })
                                    .collect();
                            }

                            ConnectState::ConnectedWait
                        }
                        other => {
                            warn!("Cannot join server with error \"{:?}\"", other);
                            ConnectState::Closed
                        }
                    });
                }
                NetServerMessage::Game(..) => {
                    error!("Received game message when not in game!")
                }
            }
        }
        None
    }

    pub(crate) fn gui_receive<'d, ID: Default + Eq + Hash + Debug + Clone + DeserializeOwned>(
        &mut self,
        gui: &mut BattlePlayerGui<ID, &'d Pokemon, &'d Move, &'d Item>,
        player: &mut GuiPlayer<'d>,
        endpoint: &mut MpscEndpoint<ID>,
        ctx: &mut GameContext,
        state: &mut ConnectState,
    ) {
        while let Some(message) = self.recv() {
            match message {
                NetServerMessage::Game(message) => {
                    debug!("received message {:?}", message);

                    match &message {
                        ServerMessage::Begin(..) => {
                            *state = ConnectState::ConnectedPlay;
                            let npc = "rival".parse().unwrap();
                            for r in gui.remotes.values_mut() {
                                r.npc_group = Some(npc);
                            }
                            gui.start(true);
                        }
                        ServerMessage::PlayerEnd(..) | ServerMessage::GameEnd(..) => {
                            *state = ConnectState::Closed;
                        }
                        _ => (),
                    }

                    endpoint.send(message); // give gui the message
                    let pokedex = unsafe { crate::POKEDEX.as_ref().unwrap() };
                    let movedex = unsafe { crate::MOVEDEX.as_ref().unwrap() };
                    let itemdex = unsafe { crate::ITEMDEX.as_ref().unwrap() };
                    gui.process(
                        &mut ctx.random,
                        &ctx.dex,
                        &ctx.btl,
                        pokedex,
                        movedex,
                        itemdex,
                        &mut player.party,
                    ); // process messages
                }
                NetServerMessage::Validate(message) => {
                    warn!("Received client validation message \"{:?}\"", message);
                    *state = ConnectState::WrongVersion(5.0);
                }
            }
        }
    }

    pub fn gui_send<ID: Serialize>(&mut self, endpoint: &mut MpscEndpoint<ID>) {
        while let Ok(message) = BattleEndpoint::receive(endpoint) {
            self.send(&NetClientMessage::Game(message));
        }
    }

    pub fn send<ID: Serialize>(&mut self, message: &NetClientMessage<ID>) {
        match serialize(message) {
            Ok(bytes) => self.sender.send(bytes),
            Err(err) => todo!("{}", err),
        }
    }

    pub fn recv<ID: DeserializeOwned>(&mut self) -> Option<NetServerMessage<ID>> {
        match self.receiver.receive() {
            Ok(Some(packet)) => match deserialize::<NetServerMessage<ID>>(packet.payload()) {
                Ok(message) => Some(message),
                Err(err) => {
                    warn!("Could not receive server message with error {}", err);
                    None
                }
            },
            _ => None,
        }
    }
}
