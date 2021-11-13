extern crate firecore_battle_net as common;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use log::{debug, error, info, warn, LevelFilter};
use rand::prelude::ThreadRng;
use simple_logger::SimpleLogger;
use std::collections::HashMap;

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
        BasicDex, Dex,
    },
    serialize as serialize2, ConnectMessage, Id, NetClientMessage, NetServerMessage, VERSION,
};

use crate::{
    configuration::Configuration,
    player::{generate_party, BattleServerPlayer},
};

mod configuration;
mod net;
mod player;

use net::*;

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

    let (bmoves, scripts) = deserialize::<(
        std::collections::HashMap<MoveId, MoveExecution>,
        std::collections::HashMap<MoveId, String>,
    )>(include_bytes!("../battle.bin"))
    .unwrap_or_else(|err| panic!("Could not deserialize battle moves with error {}", err));

    let mut engine = DefaultMoveEngine::new::<Id, ThreadRng>();

    engine.scripting.scripts = scripts;

    engine.moves = bmoves;

    let mut random = rand::thread_rng();

    // Initialize networking

    debug!("Attempting to listen on port: {}", configuration.port);

    let socket = Socket::new(configuration.port);

    info!("Listening on port {}", configuration.port);

    let sender = socket.sender();
    let mut receiver = socket.receiver();

    // Waiting room

    let mut players = HashMap::with_capacity(2);
    let mut parties = HashMap::with_capacity(2);

    while players.values().flatten().count() < 2 {
        match receiver.receive() {
            Some(packet) => match deserialize::<NetClientMessage<Id>>(packet.payload()) {
                Ok(message) => match message {
                    NetClientMessage::RequestJoin(version) => {
                        let party = generate_party(&mut random, pokedex.len() as _);
                        if players.insert(packet.address(), None).is_some() {
                            error!(
                                "Player at {} was replaced with another connection!",
                                packet.address(),
                            );
                        } else {
                            info!("Player joined at {}", packet.address());
                            parties.insert(packet.address(), party.clone());
                        }
                        sender.send(
                            packet.address(),
                            serialize(&NetServerMessage::<Id>::Validate(
                                match version == VERSION {
                                    true => ConnectMessage::CanJoin(party),
                                    false => ConnectMessage::WrongVersion,
                                },
                            )),
                        );
                    }
                    NetClientMessage::Join(player) => match players.get_mut(&packet.address()) {
                        Some(p) => *p = Some(player),
                        None => sender.send(
                            packet.address(),
                            serialize(&NetServerMessage::<Id>::Validate(
                                ConnectMessage::AlreadyConnected,
                            )),
                        ),
                    },
                    NetClientMessage::Game(..) => {
                        warn!("Endpoint at {} is sending game messages", packet.address())
                    }
                    NetClientMessage::Leave => {
                        info!("Player left at {}", packet.address());
                        players.remove(&packet.address());
                    }
                },
                Err(err) => warn!("Could not deserialize message with error {}", err),
            },
            None => (),
        }
    }

    // Create battle

    info!("Starting battle.");

    let mut receivers = HashMap::with_capacity(players.len());

    let players = players
        .into_iter()
        .enumerate()
        .flat_map(|(index, (endpoint, player))| match player {
            Some(player) => {
                let (cs, cr) = crossbeam_channel::unbounded();
                receivers.insert(endpoint, cs);
                Some(PlayerData {
                    id: index as u8,
                    name: Some(player.name),
                    party: parties.remove(&endpoint).unwrap(),
                    settings: Default::default(),
                    endpoint: BattleServerPlayer::new(endpoint, &sender, cr),
                })
            }
            None => {
                sender.send(
                    endpoint,
                    serialize(&NetServerMessage::<Id>::Validate(
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

    // loop {
    //     processor.process_poll_event(None, |event| match event {
    //         NetEvent::Accepted(endpoint, resource_id) => {
    //             info!(
    //                 "A client ({:?}) tried to join while a game is in session.",
    //                 endpoint
    //             );
    //             controller_handle.remove(resource_id);
    //         }
    //         NetEvent::Message(endpoint, bytes) => {

    //         }
    //         NetEvent::Disconnected(endpoint) => {
    //             info!("Endpoint at {} disconnected.", endpoint);
    //             running_handle.store(false, Ordering::Relaxed);
    //         }
    //         NetEvent::Connected(..) => (),
    //     });
    // });

    while !battle.finished() {
        while let Some(packet) = receiver.receive() {
            match deserialize::<NetClientMessage<Id>>(packet.payload()) {
                Ok(message) => match message {
                    NetClientMessage::Game(message) => {
                        match receivers.get(&packet.address()) {
                            Some(channel) => {
                                if let Err(err) = channel.try_send(message) {
                                    log::error!("Could not send over channel with error {}", err);
                                }
                            }
                            None => log::error!("Could not find endpoint at {}", packet.address()),
                        }
                        // get_endpoint(&receiver_handle, &endpoint).push(message)
                    }
                    NetClientMessage::RequestJoin(..) | NetClientMessage::Join(..) => sender.send(
                        packet.address(),
                        serialize(&NetServerMessage::<Id>::Validate(
                            ConnectMessage::InProgress,
                        )),
                    ),
                    NetClientMessage::Leave => {
                        info!("Endpoint at {} disconnected.", packet.address());
                        running.store(false, Ordering::Relaxed);
                    }
                },
                Err(err) => error!("Could not deserialize message with error {}", err),
            }
        }
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
