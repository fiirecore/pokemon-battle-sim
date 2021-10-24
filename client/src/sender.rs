use log::{debug, info, warn};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, hash::Hash, net::SocketAddr, sync::Arc};

use common::{
    battle::{
        endpoint::{BattleEndpoint, MpscEndpoint},
        message::ServerMessage,
    },
    deserialize,
    net::network::{split, Endpoint, NetEvent, NetworkController, SendStatus},
    pokedex::{
        item::{bag::Bag, Item, SavedItemStack},
        pokemon::{
            owned::{OwnedPokemon, SavedPokemon},
            party::Party,
            stat::StatSet,
        },
        Dex,
    },
    rand::Rng,
    serialize,
    sync::Mutex,
    ConnectMessage, NetClientMessage, NetServerMessage, Player, Queue, VERSION,
};

use gui::{BattlePlayerGui, pokedex::context::PokedexClientContext};

use crate::{ConnectState, GameContext};

type MessageQueue<ID, const AS: usize> = Arc<Mutex<Queue<NetServerMessage<ID, AS>>>>;

pub struct BattleConnection<
    'd,
    ID: Default + Clone + Eq + Hash + Debug + DeserializeOwned + Serialize + Send + 'static,
    const AS: usize,
> {
    controller: NetworkController,
    endpoint: Endpoint,
    messages: MessageQueue<ID, AS>,
    pub party: Party<OwnedPokemon<'d>>,
    pub bag: Bag<'d>,
    name: Option<String>,
    accumulator: f32,
}

impl<
        'd,
        ID: Default + Clone + Eq + Hash + Debug + DeserializeOwned + Serialize + Send + 'static,
        const AS: usize,
    > BattleConnection<'d, ID, AS>
{
    pub fn connect(itemdex: &'d dyn Dex<Item>, address: SocketAddr, name: Option<String>) -> Self {
        let (controller, mut processor) = split();

        info!("Connecting to {}", address);

        let (server, ..) = controller
            .connect(common::PROTOCOL, address)
            .unwrap_or_else(|err| panic!("Could not connect to {} with error {}", address, err));

        let messages: MessageQueue<ID, AS> = Default::default();

        let receiver = messages.clone();

        std::thread::spawn(move || loop {
            processor.process_poll_event(None, |event| match event {
                NetEvent::Connected(..) => (),
                NetEvent::Accepted(endpoint, id) => {
                    debug!("Accepted to endpoint: {} with resource id {}", endpoint, id)
                }
                NetEvent::Message(endpoint, bytes) => {
                    if endpoint == server {
                        match deserialize::<NetServerMessage<ID, AS>>(&bytes) {
                            Ok(message) => {
                                debug!("Received message: {:?}", message);
                                receiver.lock().push(message);
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
            });
        });

        Self {
            controller,
            endpoint: server,
            messages,
            name,
            party: Default::default(),
            bag: Bag::init(itemdex, vec![SavedItemStack::new("hyper_potion".parse().unwrap(), 2)]),
            accumulator: 9.9,
        }
    }

    pub fn end(&mut self) {
        self.controller.remove(self.endpoint.resource_id());
    }

    pub fn wait_confirm<R: Rng>(
        &mut self,
        random: &mut R,
        ctx: &PokedexClientContext<'d>,
        delta: f32,
    ) -> Option<ConnectState> {
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

                            let party = generate_party(random, ctx.pokedex.len() as _);

                            let name = self.name.take().unwrap_or_else(|| {
                                use common::rand::distributions::Alphanumeric;
                                std::iter::repeat(())
                                    .map(|()| random.sample(Alphanumeric))
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
                                        o.init(random, ctx.pokedex, ctx.movedex, ctx.itemdex)
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
        gui: &mut BattlePlayerGui<'d, ID, AS>,
        endpoint: &mut MpscEndpoint<ID, AS>,
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
                            gui.start(true);
                        }
                        // ServerMessage::End(..) => {
                        //     *state = ConnectState::Closed;
                        // }
                        _ => (),
                    }

                    endpoint.send(message); // give gui the message
                    gui.process(&mut ctx.random, &ctx.dex, &mut self.party); // process messages
                }
                NetServerMessage::Validate(message) => {
                    warn!("Received client validation message \"{:?}\"", message);
                    *state = ConnectState::WrongVersion(5.0);
                }
            }
        }
    }

    pub fn gui_send(&mut self, endpoint: &mut MpscEndpoint<ID, AS>) {
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

    pub fn recv(&mut self) -> Option<NetServerMessage<ID, AS>> {
        self.messages.lock().pop()
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
