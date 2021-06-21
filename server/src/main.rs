extern crate firecore_battle_net as common;

use dashmap::DashMap;
use simple_logger::SimpleLogger;

use std::{
    collections::VecDeque,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use common::{
    battle::{
        data::{BattleData, BattleType},
        message::ClientMessage,
        Battle,
    },
    hash::HashMap,
    log::{debug, error, info, warn, LevelFilter},
    net::network::SendStatus,
    net::network::{split, Endpoint, NetEvent, NetworkController},
    pokedex::moves::usage::script::engine,
    ser,
    NetClientMessage, NetServerMessage,
};

use crate::{configuration::Configuration, player::BattleServerPlayer};

type Receiver = DashMap<Endpoint, VecDeque<ClientMessage>>;

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

    pokedex_init_mini(
        ser::deserialize(include_bytes!("../dex.bin"))
            .unwrap_or_else(|err| panic!("Could not deserialize pokedex with error {}", err)),
    );

    // Initialize networking

    debug!("Attempting to listen on port: {}", configuration.port);

    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), configuration.port);

    let (controller, mut processor) = split();

    controller
        .listen(common::PROTOCOL, address)
        .unwrap_or_else(|err| {
            panic!(
                "Could not listen on network address {} with error {}",
                address, err
            )
        });

    info!("Listening on port {}", configuration.port);

    let mut players = HashMap::with_capacity(2);
    let engine = engine();

    // Waiting room

    while players.len() < 2 {
        processor.process_poll_events_until_timeout(
            Duration::from_millis(5),
            |event| match event {
                NetEvent::Accepted(endpoint, ..) => {
                    info!("Client connected from endpoint {}", endpoint);
                    send(
                        &controller,
                        endpoint,
                        &ser::serialize(&NetServerMessage::CanConnect(true)).unwrap(),
                    );
                }
                NetEvent::Message(endpoint, bytes) => {
                    match ser::deserialize::<NetClientMessage>(bytes) {
                        Ok(message) => match message {
                            NetClientMessage::Connect(player) => {
                                info!("Endpoint at {} has sent player data.", endpoint);
                                if players.insert(endpoint, player).is_some() {
                                    error!(
                                        "Player at {} was replaced with another connection!",
                                        endpoint
                                    );
                                    return;
                                }
                            }
                            NetClientMessage::Game(..) => todo!(),
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

    let receiver = Arc::new(DashMap::new());

    let mut players = players.into_iter();

    let controller = Arc::new(controller);

    let mut battle = Battle::new(
        BattleData {
            type_: BattleType::Trainer,
        },
        BattleServerPlayer::player(
            players.next().unwrap(),
            controller.clone(),
            receiver.clone(),
        ),
        BattleServerPlayer::player(
            players.next().unwrap(),
            controller.clone(),
            receiver.clone(),
        ),
    );

    battle.begin();

    let running = Arc::new(AtomicBool::new(true));

    // Queue close on control-c

    let running_handle = running.clone();

    let receiver_handle = receiver.clone();

    let controller_handle = controller.clone();

    ctrlc::set_handler(move || {
        end_battle(
            running_handle.as_ref(),
            receiver_handle.as_ref(),
            controller_handle.as_ref(),
        )
    })
    .unwrap();

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
                match ser::deserialize::<NetClientMessage>(bytes) {
                    Ok(message) => match message {
                        NetClientMessage::Game(message) => receiver_handle
                            .get_mut(&endpoint)
                            .unwrap()
                            .push_back(message),
                        NetClientMessage::Connect(..) => todo!("Client reconnecting."),
                    },
                    Err(err) => warn!("Could not deserialize message with error {}", err),
                }
            }
            NetEvent::Disconnected(endpoint) => {
                info!("Endpoint at {} disconnected.", endpoint);
                running_handle.store(false, Ordering::Relaxed);
            }
            NetEvent::Connected(..) => (),
        });
    });

    {
        // Send signal to begin battle
        let message = &ser::serialize(&NetServerMessage::Begin).unwrap();
        for endpoint in receiver.iter() {
            send(&controller, *endpoint.key(), message);
        }
    }

    while !battle.finished() && running.load(Ordering::Relaxed) {
        battle.update(&engine);
        thread::sleep(Duration::from_millis(5)); // To - do: only process when messages are received, stay idle and dont loop when not received
    }

    end_battle(running.as_ref(), receiver.as_ref(), &controller);

    info!("closing server.");
}

fn send(controller: &NetworkController, endpoint: Endpoint, data: &[u8]) {
    match controller.send(endpoint, data) {
        SendStatus::Sent => (),
        status => error!("Could not send message with error {:?}", status),
    }
}

use common::pokedex::{
    item::{Item, Itemdex},
    moves::{Move, Movedex},
    pokemon::{Pokedex, Pokemon},
    Dex,
};

pub fn pokedex_init_mini(dex: (Vec<Pokemon>, Vec<Move>, Vec<Item>)) {
    Pokedex::set(dex.0.into_iter().map(|p| (p.id, p)).collect());
    Movedex::set(dex.1.into_iter().map(|m| (m.id, m)).collect());
    Itemdex::set(dex.2.into_iter().map(|i| (i.id, i)).collect());
}

fn end_battle(running: &AtomicBool, receiver: &Receiver, controller: &NetworkController) {
    let data = &ser::serialize(&NetServerMessage::End).unwrap();
    for endpoint in receiver.iter() {
        controller.send(*endpoint.key(), data);
    }
    running.store(false, Ordering::Relaxed);
}
