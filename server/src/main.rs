extern crate firecore_battle_net as common;

use simple_logger::SimpleLogger;

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use log::{debug, error, info, warn, LevelFilter};

use hashbrown::HashMap;

use message_io::network::{Endpoint, NetEvent, NetworkController, SendStatus, Transport, split};

use common::{
    battle::{
        engine::default::moves::MoveExecution,
        prelude::{Battle, BattleData, BattleType, DefaultMoveEngine, PlayerData},
    },
    deserialize,
    pokedex::{
        item::Item,
        moves::{Move, MoveId},
        pokemon::Pokemon,
        BasicDex,
    },
    rand::prelude::ThreadRng,
    serialize as serialize2, ConnectMessage, Id, NetClientMessage, NetServerMessage,
    VERSION,
};

use crate::{configuration::Configuration, player::BattleServerPlayer};

mod configuration;
mod player;

fn main() {
    // Initialize logger

    let logger = SimpleLogger::new();

    #[cfg(debug_assertions)]
    let logger = logger.with_level(LevelFilter::Debug);
    #[cfg(not(debug_assertions))]
    let logger = logger.with_level(LevelFilter::Info);

    logger
        .init()
        .unwrap_or_else(|err| panic!("Could not initialize logger with error {}", err));

    // Load configuration

    let configuration = Configuration::load();

    info!("Successfully loaded configuration.");

    // Initialize pokemon

    let (pokedex, movedex, itemdex) =
        deserialize::<(BasicDex<Pokemon>, BasicDex<Move>, BasicDex<Item>)>(include_bytes!(
            "../../dex.bin"
        ))
        .unwrap_or_else(|err| panic!("Could not deserialize dexes with error {}", err));

    let (bmoves, scripts) =
        deserialize::<(HashMap<MoveId, MoveExecution>, HashMap<MoveId, String>)>(include_bytes!(
            "../battle.bin"
        ))
        .unwrap_or_else(|err| panic!("Could not deserialize battle moves with error {}", err));

    // Initialize networking

    debug!("Attempting to listen on port: {}", configuration.port);

    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), configuration.port);

    let (controller, mut processor) = split();

    controller
        .listen(Transport::FramedTcp, address)
        .unwrap_or_else(|err| {
            panic!(
                "Could not listen on network address {} with error {}",
                address, err
            )
        });

    info!("Listening on port {}", configuration.port);

    let mut players = HashMap::with_capacity(2);

    let mut engine = DefaultMoveEngine::new::<Id, ThreadRng>();

    engine.scripting.scripts = scripts;

    engine.moves = bmoves;

    let mut random = common::rand::thread_rng();

    // for ai in 0..configuration.ai {
    //     players.insert(
    //         PlayerKind::AI(ai),
    //         Some(Player {
    //             name: format!("AI {}", ai),
    //             party: common::generate_party(&mut random, pokedex.len() as _),
    //         }),
    //     );
    // }

    // Waiting room

    while players.values().flatten().count() < 2 {
        processor.process_poll_events_until_timeout(
            Duration::from_millis(5),
            |event| match event {
                NetEvent::Accepted(endpoint, ..) => {
                    info!("Client connected from endpoint {}", endpoint)
                }
                NetEvent::Message(endpoint, bytes) => {
                    match deserialize::<NetClientMessage<Id>>(bytes) {
                        Ok(message) => match message {
                            NetClientMessage::RequestJoin(version) => {
                                send(
                                    &controller,
                                    endpoint,
                                    &serialize(&NetServerMessage::<Id>::Validate(
                                        match version == VERSION {
                                            true => ConnectMessage::CanJoin,
                                            false => ConnectMessage::WrongVersion,
                                        },
                                    )),
                                );
                                if players.insert(endpoint, None).is_some() {
                                    error!(
                                        "Player at {} was replaced with another connection!",
                                        endpoint
                                    );
                                }
                            }
                            NetClientMessage::Join(player) => match players.get_mut(&endpoint) {
                                Some(p) => *p = Some(player),
                                None => send(
                                    &controller,
                                    endpoint,
                                    &serialize(&NetServerMessage::<Id>::Validate(
                                        ConnectMessage::AlreadyConnected,
                                    )),
                                ),
                            },
                            NetClientMessage::Game(..) => {
                                warn!("Endpoint at {} is sending game messages", endpoint)
                            }
                        },
                        Err(err) => warn!("Could not deserialize message with error {}", err),
                    }
                }
                NetEvent::Connected(endpoint, ..) => info!("Endpoint at {} connected.", endpoint),
                NetEvent::Disconnected(endpoint) => {
                    players.remove(&endpoint);
                    info!("Endpoint at {} disconnected.", endpoint);
                }
            },
        );
    }

    // Create battle

    info!("Starting battle.");

    let mut receiver = HashMap::with_capacity(players.len());

    let controller = Arc::new(controller);

    let players = players
        .into_iter()
        .enumerate()
        .flat_map(|(index, (endpoint, player))| match player {
            Some(player) => {
                let (cs, cr) = crossbeam_channel::unbounded();
                receiver.insert(endpoint, cs);
                Some(PlayerData {
                id: index as u8,
                name: Some(player.name),
                party: player.party,
                settings: Default::default(),
                endpoint: BattleServerPlayer::new(endpoint, &controller, cr),
            })
        },
            None => {
                send(
                    &controller,
                    endpoint,
                    &serialize(&NetServerMessage::<Id>::Validate(
                        ConnectMessage::ConnectionReplaced,
                    )),
                );
                None
            }
        });

    let mut battle = Battle::new(
        BattleData {
            type_: BattleType::Trainer,
        },
        &mut random,
        configuration.battle_size as _,
        &pokedex,
        &movedex,
        &itemdex,
        players,
    );

    battle.begin();

    let running = Arc::new(AtomicBool::new(true));

    // Queue close on control-c

    let running_handle = running.clone();

    ctrlc::set_handler(move || running_handle.store(false, Ordering::Relaxed))
        .unwrap_or_else(|err| panic!("Could not set Ctrl + C handler with error {}", err));

    // Handle incoming messages

    let running_handle = running.clone();

    let receiver_handle = receiver.clone();

    let controller_handle = controller.clone();

    thread::spawn(move || loop {
        processor.process_poll_event(None, |event| match event {
            NetEvent::Accepted(endpoint, resource_id) => {
                info!(
                    "A client ({:?}) tried to join while a game is in session.",
                    endpoint
                );
                controller_handle.remove(resource_id);
            }
            NetEvent::Message(endpoint, bytes) => {
                match deserialize::<NetClientMessage<Id>>(bytes) {
                    Ok(message) => match message {
                        NetClientMessage::Game(message) => {
                            match receiver_handle.get(&endpoint) {
                                Some(channel) => if let Err(err) = channel.try_send(message) {
                                    log::error!("Could not send over channel with error {}", err);
                                }
                                None => log::error!("Could not find endpoint at {}", endpoint),
                            }
                            // get_endpoint(&receiver_handle, &endpoint).push(message)
                        }
                        NetClientMessage::RequestJoin(..) | NetClientMessage::Join(..) => send(
                            &controller,
                            endpoint,
                            &serialize(&NetServerMessage::<Id>::Validate(
                                ConnectMessage::InProgress,
                            )),
                        ),
                    },
                    Err(err) => error!("Could not deserialize message with error {}", err),
                }
            }
            NetEvent::Disconnected(endpoint) => {
                info!("Endpoint at {} disconnected.", endpoint);
                running_handle.store(false, Ordering::Relaxed);
            }
            NetEvent::Connected(..) => (),
        });
    });

    while !battle.finished() {
        if !running.load(Ordering::Relaxed) {
            battle.end(None);
        }
        battle.update(&mut random, &mut engine, &movedex, &itemdex);
        thread::sleep(Duration::from_millis(5)); // To - do: only process when messages are received, stay idle and dont loop when not received
    }

    info!("closing server.");
}

// #[derive(PartialEq, Eq, Hash)]
// enum PlayerKind {
//     Endpoint(Endpoint),
//     AI(u8),
// }

fn send(controller: &NetworkController, endpoint: Endpoint, data: &[u8]) {
    match controller.send(endpoint, data) {
        SendStatus::Sent => (),
        status => error!("Could not send message with error {:?}", status),
    }
}

// fn get_endpoint<'a, ID>(
//     receiver: &'a Receiver<ID>,
//     endpoint: &Endpoint,
// ) -> RefMut<'a, Endpoint, Queue<ClientMessage<ID>>, RandomState> {
//     receiver
//         .get_mut(&endpoint)
//         .unwrap_or_else(|| panic!("Could not get message queue for endpoint {}", endpoint))
// }

fn serialize(s: &impl serde::Serialize) -> Vec<u8> {
    serialize2(s).unwrap()
}
