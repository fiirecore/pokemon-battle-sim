use std::{net::SocketAddr, time::Instant};

use common::{
    game::{
        battle::{
            client::{BattleClient, BattleEndpoint},
            message::ServerMessage,
        },
        battle_cli::clients::gui::BattlePlayerGui,
        deps::ser,
        log::info,
        pokedex::{
            Dex,
            pokemon::{
                Pokedex,
                instance::PokemonInstance,
                party::PokemonParty,
                stat::StatSet,
                PokemonId
            },
            trainer::TrainerData,
        },
        tetra::Context,
        util::{date, Entity},
    },
    laminar::{Packet, Socket, SocketEvent},
    NetBattleClient, NetClientMessage, NetServerMessage, Player, SERVER_PORT,
};

use rand::Rng;

pub struct BattleConnection {
    socket: Socket,
    server: SocketAddr,
    client: SocketAddr,
    name: Option<String>,
}

impl BattleConnection {
    pub fn connect(address: SocketAddr, name: Option<String>) -> Self {
        let server = SocketAddr::new(address.ip(), SERVER_PORT);
        let client = SocketAddr::new(
            common::ip().unwrap(),
            address.port(),
        );

        let mut socket = Socket::bind(client).unwrap();
        info!("Connected on {}", client);

        socket
            .send(Packet::reliable_unordered(
                server,
                ser::serialize(&NetClientMessage::RequestConnect).unwrap(),
            ))
            .unwrap();
        info!("Sent connection request to server.");

        Self {
            socket,
            server,
            client,
            name,
        }
    }

    pub fn poll(&mut self) {
        self.socket.manual_poll(Instant::now());
    }

    pub fn wait_confirm(&mut self) -> bool {
        if let Some(event) = self.socket.recv() {
            match event {
                SocketEvent::Packet(packet) => {
                    if let Ok(message) = ser::deserialize(packet.payload()) {
                        match message {
                            NetServerMessage::AcceptConnect => {
                                info!("Server accepted connection!");

                                let name = date().to_string();

                                let mut pokemon = PokemonParty::new();

                                let mut rand = rand::thread_rng();

                                for _ in 0..pokemon.capacity() {
                                    let id = rand.gen_range(1..Pokedex::len() as PokemonId);
                                    pokemon.push(PokemonInstance::generate_with_level(
                                        id,
                                        50,
                                        Some(StatSet::uniform(15)),
                                    ));
                                }

                                self.socket.send(Packet::reliable_unordered(
                                self.server,
                                ser::serialize(
                                    &NetClientMessage::Connect(
                                        Player {
                                            id: name.parse().unwrap(),
                                            trainer: TrainerData {
                                                npc_type: "rival".parse().unwrap(),
                                                prefix: "Trainer".to_owned(),
                                                name: self.name.take().unwrap_or(name),
                                            },
                                            party: pokemon,
                                            client: NetBattleClient(self.client),
                                        }
                                    )
                                ).unwrap_or_else(|err| panic!("Could not send connect message to server with error {}", err))
                            )).unwrap();
                                return true;
                            }
                        }
                    }
                }
                _ => (),
            }
        }
        false
    }

    pub fn receive(&mut self, gui: &mut BattlePlayerGui, ctx: &mut Context) {
        while let Some(event) = self.socket.recv() {
            match event {
                SocketEvent::Packet(packet) => {
                    if let Ok(message) = ser::deserialize::<ServerMessage>(packet.payload()) {
                        let message_eq = matches!(message, ServerMessage::Opponents(..));
                        gui.give_client(message);
                        if message_eq {
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
                    }
                }
                _ => (),
            }
        }
    }

    pub fn send(&mut self, gui: &mut BattlePlayerGui) {
        while let Some(message) = gui.give_server() {
            match ser::serialize(&message) {
                Ok(bytes) => {
                    if let Err(err) = self.socket.send(Packet::reliable_unordered(self.server, bytes)) {
                        todo!("{}", err)
                    }
                }
                Err(err) => todo!("{}", err),
            }
        }
    }
}
