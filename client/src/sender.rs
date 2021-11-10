use crossbeam_channel::Receiver;
use log::{debug, info, warn};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, hash::Hash, net::SocketAddr};

use message_io::network::{split, Endpoint, NetEvent, NetworkController, SendStatus, Transport};

use common::{
    battle::{
        endpoint::{BattleEndpoint, MpscEndpoint},
        message::ServerMessage,
    },
    deserialize,
    pokedex::{
        item::{bag::OwnedBag, Item, SavedItemStack},
        moves::Move,
        pokemon::{
            owned::{OwnedPokemon, SavedPokemon},
            party::Party,
            stat::StatSet,
            Pokemon,
        },
        Dex,
    },
    rand::Rng,
    serialize, ConnectMessage, NetClientMessage, NetServerMessage, Player, VERSION,
};

use gui::{
    pokedex::{BasicDex, Initializable},
    BattlePlayerGui,
};

use crate::{ConnectState, GameContext};

pub struct BattleConnection<
    'd,
    ID: Default + Clone + Eq + Hash + Debug + DeserializeOwned + Serialize + Send + 'static,
> {
    controller: NetworkController,
    endpoint: Endpoint,
    receiver: Receiver<NetServerMessage<ID>>,
    pub party: Party<OwnedPokemon<&'d Pokemon, &'d Move, &'d Item>>,
    pub bag: OwnedBag<&'d Item>,
    name: Option<String>,
    accumulator: f32,
}

impl<
        'd,
        ID: Default + Clone + Eq + Hash + Debug + DeserializeOwned + Serialize + Send + 'static,
    > BattleConnection<'d, ID>
{
    pub fn connect(itemdex: &'d BasicDex<Item>, address: SocketAddr, name: Option<String>) -> Self {
        let (controller, mut processor) = split();

        info!("Connecting to {}", address);

        let (server, ..) = controller
            .connect(Transport::FramedTcp, address)
            .unwrap_or_else(|err| panic!("Could not connect to {} with error {}", address, err));

        let (sender, receiver) = crossbeam_channel::unbounded();

        std::thread::spawn(move || loop {
            processor.process_poll_event(Some(std::time::Duration::from_millis(1)), |event| {
                match event {
                    NetEvent::Connected(..) => (),
                    NetEvent::Accepted(endpoint, id) => {
                        debug!("Accepted to endpoint: {} with resource id {}", endpoint, id)
                    }
                    NetEvent::Message(endpoint, bytes) => {
                        if endpoint == server {
                            match deserialize::<NetServerMessage<ID>>(&bytes) {
                                Ok(message) => {
                                    debug!("Received message: {:?}", message);
                                    if let Err(err) = sender.try_send(message) {
                                        log::error!("Cannot send message through MPSC channel with error {}", err);
                                    }
                                }
                                Err(err) => {
                                    warn!("Could not receive server message with error {}", err)
                                }
                            }
                        } else {
                            warn!("Received packets from non server endpoint!")
                        }
                    }
                    NetEvent::Disconnected(endpoint) => {
                        info!("Disconnected from endpoint: {}", endpoint)
                    }
                }
            });
        });

        Self {
            controller,
            endpoint: server,
            receiver,
            name,
            party: Default::default(),
            bag: vec![SavedItemStack::new("hyper_potion".parse().unwrap(), 2)]
                .init(itemdex)
                .unwrap(),
            accumulator: 9.9,
        }
    }

    pub fn end(&mut self) {
        self.controller.remove(self.endpoint.resource_id());
    }

    pub fn wait_confirm(&mut self, ctx: &'d mut GameContext, delta: f32) -> Option<ConnectState> {
        self.accumulator += delta;
        if self.accumulator >= 10.0 {
            self.controller.send(
                self.endpoint,
                &serialize(&NetClientMessage::<ID>::RequestJoin(VERSION.to_owned())).unwrap(),
            );
            self.accumulator -= 10.0;
        }
        if let Some(message) = self.recv() {
            match message {
                NetServerMessage::Validate(message) => {
                    return Some(match message {
                        ConnectMessage::CanJoin => {
                            info!("Server accepted connection!");

                            let pokedex = unsafe { crate::POKEDEX.as_ref().unwrap() };

                            let party = generate_party(&mut ctx.random, pokedex.len() as _);

                            let name = self.name.take().unwrap_or_else(|| {
                                use common::rand::distributions::Alphanumeric;
                                std::iter::repeat(())
                                    .map(|()| ctx.random.sample(Alphanumeric))
                                    .map(char::from)
                                    .take(7)
                                    .collect()
                            });

                            let m = NetClientMessage::Join(Player { name, party });

                            self.send(&m);

                            if let NetClientMessage::Join(p) = m {
                                self.party = p
                                    .party
                                    .into_iter()
                                    .map(|o| {
                                        let pokedex = unsafe { crate::POKEDEX.as_ref().unwrap() };
                                        let movedex = unsafe { crate::MOVEDEX.as_ref().unwrap() };
                                        let itemdex = unsafe { crate::ITEMDEX.as_ref().unwrap() };
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
                other => warn!("Server sent unusable message {:?}", other),
            }
        }
        None
    }

    pub fn gui_receive(
        &mut self,
        gui: &mut BattlePlayerGui<ID, &'d Pokemon, &'d Move, &'d Item>,
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
                        &mut self.party,
                    ); // process messages
                }
                NetServerMessage::Validate(message) => {
                    warn!("Received client validation message \"{:?}\"", message);
                    *state = ConnectState::WrongVersion(5.0);
                }
            }
        }
    }

    pub fn gui_send(&mut self, endpoint: &mut MpscEndpoint<ID>) {
        while let Ok(message) = BattleEndpoint::receive(endpoint) {
            self.send(&NetClientMessage::Game(message));
        }
    }

    pub fn send(&mut self, message: &NetClientMessage<ID>) {
        debug!("Sending message {:?}", message);
        match serialize(message) {
            Ok(bytes) => match self.controller.send(self.endpoint, &bytes) {
                SendStatus::Sent => (),
                status => todo!("{:?}", status),
            },
            Err(err) => todo!("{}", err),
        }
    }

    pub fn recv(&mut self) -> Option<NetServerMessage<ID>> {
        self.receiver.try_recv().ok()
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
