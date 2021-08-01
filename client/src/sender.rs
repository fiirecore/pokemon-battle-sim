use log::{debug, info, warn};
use std::{net::SocketAddr, sync::Arc};

use common::{
    Queue,
    battle::BattleEndpoint,
    net::network::{split, Endpoint, NetEvent, NetworkController, SendStatus},
    pokedex::{
        id::Dex,
        pokemon::{stat::StatSet, Pokedex, PokemonId, PokemonInstance, PokemonParty},
    },
    rand::Rng,
    ser,
    sync::Mutex,
    uuid::Uuid,
    NetClientMessage, NetServerMessage, Player,
};

use gui::BattlePlayerGui;

use crate::{ConnectState, GameContext};

type MessageQueue = Arc<Mutex<Queue<NetServerMessage>>>;

pub struct BattleConnection {
    controller: NetworkController,
    endpoint: Endpoint,
    messages: MessageQueue,
    name: Option<String>,
}

impl BattleConnection {
    pub fn connect(address: SocketAddr, name: Option<String>) -> Self {
        let (controller, mut processor) = split();

        info!("Connecting to {}", address);

        let (server, ..) = controller
            .connect(common::PROTOCOL, address)
            .unwrap_or_else(|err| panic!("Could not connect to {} with error {}", address, err));

        let messages: MessageQueue = Default::default();

        let receiver = messages.clone();

        std::thread::spawn(move || loop {
            processor.process_poll_event(None, |event| match event {
                NetEvent::Connected(..) => (),
                NetEvent::Accepted(endpoint, id) => {
                    debug!("Accepted to endpoint: {} with resource id {}", endpoint, id)
                }
                NetEvent::Message(endpoint, bytes) => {
                    if endpoint == server {
                        match ser::deserialize::<NetServerMessage>(&bytes) {
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
        }
    }

    pub fn end(&mut self) {
        self.controller.remove(self.endpoint.resource_id());
    }

    pub fn wait_confirm<R: Rng>(&mut self, random: &mut R) -> Option<ConnectState> {
        if let Some(message) = self.recv() {
            match message {
                NetServerMessage::CanConnect(accepted) => {
                    return Some(match accepted {
                        true => {
                            info!("Server accepted connection!");

                            let mut party = PokemonParty::new();

                            for _ in 0..party.capacity() {
                                let id = random.gen_range(1..Pokedex::len() as PokemonId);
                                party.push(PokemonInstance::generate_with_level(
                                    random,
                                    &id,
                                    50,
                                    Some(StatSet::uniform(15)),
                                ));
                            }

                            let name = self.name.take().unwrap_or_else(|| {
                                use common::rand::distributions::Alphanumeric;
                                std::iter::repeat(())
                                    .map(|()| random.sample(Alphanumeric))
                                    .map(char::from)
                                    .take(7)
                                    .collect()
                            });

                            self.send(&NetClientMessage::Connect(
                                Player { name, party },
                                common::VERSION,
                            ));

                            ConnectState::ConnectedWait
                        }
                        false => ConnectState::Closed,
                    });
                }
                _ => todo!(),
            }
        }
        None
    }

    pub fn gui_receive(
        &mut self,
        gui: &mut BattlePlayerGui<Uuid>,
        ctx: &mut GameContext,
        state: &mut ConnectState,
    ) {
        while let Some(message) = self.recv() {
            match message {
                NetServerMessage::Game(message) => {
                    debug!("received message {:?}", message);
                    gui.send(message); // give gui the message
                    gui.process(&ctx.pokedex); // process messages
                }
                NetServerMessage::WrongVersion => {
                    warn!("Could not connect to server as it is version incompatible!");
                    *state = ConnectState::WrongVersion(5.0);
                }
                NetServerMessage::Begin => {
                    debug!("Received begin message!");
                    *state = ConnectState::ConnectedPlay;
                    gui.start(true);
                    gui.on_begin(&ctx.pokedex);
                }
                NetServerMessage::CanConnect(..) => (),
                NetServerMessage::End => *state = ConnectState::Closed,
            }
        }
    }

    pub fn gui_send(&mut self, gui: &mut BattlePlayerGui<Uuid>) {
        while let Some(message) = BattleEndpoint::receive(gui) {
            self.send(&NetClientMessage::Game(message));
        }
    }

    pub fn send(&mut self, message: &NetClientMessage) {
        debug!("Sending message {:?}", message);
        match ser::serialize(message) {
            Ok(bytes) => match self.controller.send(self.endpoint, &bytes) {
                SendStatus::Sent => (),
                status => todo!("{:?}", status),
            },
            Err(err) => todo!("{}", err),
        }
    }

    pub fn recv(&mut self) -> Option<NetServerMessage> {
        self.messages.lock().pop()
    }
}
