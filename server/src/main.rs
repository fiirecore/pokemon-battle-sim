extern crate firecore_battle_net as common;
extern crate firecore_dependencies as deps;

use anyhow::Result;
use dashmap::DashMap;
use player::BattleServerPlayer;

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
        Battle, BattleHost,
    },
    logger::SimpleLogger,
    net::network::SendStatus,
    uuid::Uuid,
};

use deps::{
    hash::HashMap,
    log::{debug, error, info, warn, LevelFilter},
    ser,
};

use common::{
    net::network::{split, Endpoint, NetEvent, NetworkController, Transport},
    pokedex::moves::usage::script::engine,
    NetClientMessage, NetServerMessage,
};

type SharedReceiver = Arc<DashMap<Endpoint, VecDeque<ClientMessage>>>;

mod configuration;
mod player;

fn main() -> Result<()> {
    // Initialize logger

    let logger = SimpleLogger::new();

    #[cfg(debug_assertions)]
    let logger = logger.with_level(LevelFilter::Debug);
    #[cfg(not(debug_assertions))]
    let logger = logger.with_level(LevelFilter::Info);

    logger.init()?;

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

    controller.listen(Transport::Tcp, address)?;

    info!("Listening on port {}", configuration.port);

    let mut players = HashMap::with_capacity(2);
    let mut battle = Battle::<Uuid>::new(engine());

    // Waiting room

    while players.len() < 2 {
        processor.process_poll_events_until_timeout(Duration::from_millis(5), |event| {
            match event {
                NetEvent::Accepted(endpoint, _) => send(
                    &controller,
                    endpoint,
                    &ser::serialize(&NetServerMessage::CanConnect(true)).unwrap(),
                ),
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
                                // if let Err(err) = socket.send(Packet::reliable_unordered(packet.addr(), ser::serialize(&NetServerMessage::ConfirmConnect).unwrap())) {
                                //     error!("{}", err)
                                // }
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
            }
        });
    }

    // Create battle

    info!("Starting battle.");

    let receiver = Arc::new(DashMap::new());

    let mut players = players.into_iter();

    let controller = Arc::new(controller);

    battle.battle(BattleHost::new(
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
    ));

    battle.begin();

    let running = Arc::new(AtomicBool::new(true));

    // Queue close on control-c

    let running_handle = running.clone();

    let receiver_handle = receiver.clone();

    let controller_handle = controller.clone();

    ctrlc::set_handler(move || {
        let data = &ser::serialize(&NetServerMessage::End).unwrap();
        for endpoint in receiver_handle.iter() {
            controller_handle.send(*endpoint.key(), data);
        }
        running_handle.store(false, Ordering::Relaxed);
    }).unwrap();

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

    while battle.is_some() && running.load(Ordering::Relaxed) {
        battle.update();
        thread::sleep(Duration::from_millis(5)); // To - do: only process when messages are received, stay idle and dont loop when not received
    }

    {
        let message = &ser::serialize(&NetServerMessage::End).unwrap();
        for endpoint in receiver.iter() {
            send(&controller, *endpoint.key(), message);
        }
    }

    info!("closing server.");

    Ok(())
}

fn send(controller: &NetworkController, endpoint: Endpoint, data: &[u8]) {
    match controller.send(endpoint, data) {
        SendStatus::Sent => (),
        status => error!("Could not send message with error {:?}", status),
    }
}

use common::pokedex::{
    item::{Item, ItemId, Itemdex},
    moves::{Move, MoveId, Movedex},
    pokemon::{Pokedex, Pokemon, PokemonId},
    Dex,
};

use crate::configuration::Configuration;

pub fn pokedex_init_mini(
    dex: (
        HashMap<PokemonId, Pokemon>,
        HashMap<MoveId, Move>,
        HashMap<ItemId, Item>,
    ),
) {
    Pokedex::set(dex.0);
    Movedex::set(dex.1);
    Itemdex::set(dex.2);
}
