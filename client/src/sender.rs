use std::{collections::VecDeque, net::SocketAddr, sync::Arc};

use game::{
    battle::client::{BattleClient, BattleEndpoint},
    battle_cli::clients::gui::BattlePlayerGui,
    deps::ser,
    log::{debug, info, warn},
    pokedex::{
        pokemon::{
            instance::PokemonInstance, party::PokemonParty, stat::StatSet, Pokedex, PokemonId,
        },
        trainer::TrainerData,
        Dex,
    },
    tetra::Context,
    util::Entity,
};

use common::{
    net::network::{split, Endpoint, NetEvent, NetworkController, SendStatus},
    rand::Rng,
    sync::Mutex,
    uuid::Uuid,
    NetClientMessage, NetServerMessage, Player,
};

use crate::ConnectState;

pub struct BattleConnection {
    controller: NetworkController,
    endpoint: Endpoint,
    messages: Arc<Mutex<VecDeque<Vec<u8>>>>,
    name: Option<String>,
}

impl BattleConnection {
    pub fn connect(address: SocketAddr, name: Option<String>) -> Self {
        let (controller, mut processor) = split();

        let (server, address) = controller.connect(common::PROTOCOL, address).unwrap();

        info!("Connected to {}", address);

        let messages = Arc::new(Mutex::new(VecDeque::new()));

        let receiver = messages.clone();

        std::thread::spawn(move || loop {
            processor.process_poll_event(None, |event| match event {
                NetEvent::Connected(endpoint, ..) => info!("Connected to endpoint: {}", endpoint),
                NetEvent::Accepted(endpoint, id) => {
                    debug!("Accepted to endpoint: {} with resource id {}", endpoint, id)
                }
                NetEvent::Message(endpoint, bytes) => {
                    if endpoint == server {
                        receiver.lock().push_back(bytes.to_owned());
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

    pub fn wait_confirm(&mut self) -> Option<ConnectState> {
        if let Some(bytes) = self.recv() {
            match ser::deserialize::<NetServerMessage>(&bytes) {
                Ok(message) => match message {
                    NetServerMessage::CanConnect(accepted) => {
                        return Some(match accepted {
                            true => {
                                info!("Server accepted connection!");

                                let mut pokemon = PokemonParty::new();

                                let mut rand = common::rand::thread_rng();

                                for _ in 0..pokemon.capacity() {
                                    let id = rand.gen_range(1..Pokedex::len() as PokemonId);
                                    pokemon.push(PokemonInstance::generate_with_level(
                                        id,
                                        50,
                                        Some(StatSet::uniform(15)),
                                    ));
                                }

                                let npc_type = "rival".parse().unwrap();
                                let name = self.name.take().unwrap_or_else(|| {
                                    use common::rand::distributions::Alphanumeric;
                                    let mut rng = common::rand::thread_rng();
                                    std::iter::repeat(())
                                        .map(|()| rng.sample(Alphanumeric))
                                        .map(char::from)
                                        .take(7)
                                        .collect()
                                });

                                self.send(&NetClientMessage::Connect(Player {
                                    trainer: TrainerData {
                                        npc_type,
                                        prefix: "Trainer".to_owned(),
                                        name,
                                    },
                                    party: pokemon,
                                    // client: NetBattleClient(self.client),
                                }));

                                ConnectState::Connected
                            }
                            false => ConnectState::Closed,
                        });
                    }
                    _ => todo!(),
                },
                Err(err) => warn!("Could not deserialize server message with error {}", err),
            }
        }
        None
    }

    pub fn gui_receive(
        &mut self,
        gui: &mut BattlePlayerGui<Uuid>,
        ctx: &mut Context,
        state: &mut ConnectState,
    ) {
        while let Some(bytes) = self.recv() {
            match ser::deserialize::<NetServerMessage>(&bytes) {
                Ok(message) => match message {
                    NetServerMessage::Game(message) => {
                        debug!("received message {:?}", message);
                        gui.give_client(message);
                    }
                    NetServerMessage::Begin => {
                        gui.start(true);
                        gui.on_begin(ctx);
                        gui.player
                            .renderer
                            .iter_mut()
                            .for_each(|a| a.status.spawn());
                        gui.opponent
                            .renderer
                            .iter_mut()
                            .for_each(|a| a.status.spawn());
                    }
                    NetServerMessage::CanConnect(..) => (),
                    NetServerMessage::End => *state = ConnectState::Closed,
                },
                Err(err) => warn!("Could not receive server message with error {}", err),
            }
        }
    }

    pub fn gui_send(&mut self, gui: &mut BattlePlayerGui<Uuid>) {
        while let Some(message) = gui.give_server() {
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

    pub fn recv(&mut self) -> Option<Vec<u8>> {
        self.messages.lock().pop_front()
    }
}
