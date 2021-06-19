extern crate firecore_battle_net as common;

use std::{net::SocketAddr, rc::Rc};

use crossbeam_channel::{Receiver, Sender};

use common::{
    game::{
        battle::{
            client::{BattleClient, BattleEndpoint},
            data::{BattleData, BattleType},
            message::{ClientMessage, ServerMessage},
            pokemon::BattlePlayer,
            Battle, BattleHost,
        },
        deps::ser,
        log::{debug, error, info, warn},
        pokedex::{
            moves::usage::script::engine,
            pokemon::instance::BorrowedPokemon,
        },
    },
    laminar::{Packet, Socket, SocketEvent},
    NetBattleClient, NetClientMessage, NetServerMessage, Player,
};
use dashmap::DashMap;

pub fn main() {

    common::init();

    let mut players = Vec::with_capacity(2);
    let mut battle = Battle::new(engine());

    let address = SocketAddr::new("192.168.1.11".parse().unwrap(), common::SERVER_PORT);

    let mut socket = Socket::bind(address).unwrap();

    info!("Running server on {}", address);

    let sender = socket.get_packet_sender();
    let receiver = socket.get_event_receiver();

    std::thread::spawn(move || socket.start_polling());

    while players.len() < 2 {
        if let Ok(event) = receiver.recv() {
            match event {
                SocketEvent::Packet(packet) => {
                    match ser::deserialize::<NetClientMessage>(packet.payload()) {
                        Ok(message) => match message {
                            NetClientMessage::RequestConnect => {
                                info!("Accepting connection request from {}", packet.addr());
                                if let Err(err) = sender.send(Packet::reliable_unordered(
                                    packet.addr(),
                                    ser::serialize(&NetServerMessage::AcceptConnect).unwrap(),
                                )) {
                                    error!("{}", err)
                                }
                            }
                            NetClientMessage::Connect(player) => {
                                info!("Client {} has sent player data.", packet.addr());
                                players.push(player);
                                // if let Err(err) = socket.send(Packet::reliable_unordered(packet.addr(), ser::serialize(&NetServerMessage::ConfirmConnect).unwrap())) {
                                //     error!("{}", err)
                                // }
                            }
                        },
                        Err(err) => warn!("Could not deserialize message with error {}", err),
                    }
                }
                SocketEvent::Connect(a) => info!("{} connected.", a),
                SocketEvent::Timeout(a) => info!("{} timed out.", a),
                SocketEvent::Disconnect(a) => warn!("{} disconnected.", a),
            }
        }
    }

    info!("Starting battle.");

    let map = Rc::new(DashMap::new());

    battle.battle(BattleHost::new(
        BattleData {
            type_: BattleType::Trainer,
        },
        player(
            players.remove(0),
            sender.clone(),
            receiver.clone(),
            map.clone(),
        ),
        player(players.remove(0), sender.clone(), receiver.clone(), map),
    ));

    battle.begin();

    while battle.is_some() {
        battle.update();
        std::thread::sleep(std::time::Duration::from_millis(3))
    }

    info!("closing server.");
}

fn player(
    player: Player,
    sender: Sender<Packet>,
    receiver: Receiver<SocketEvent>,
    map: Rc<DashMap<SocketAddr, ClientMessage>>,
) -> BattlePlayer {
    BattlePlayer::new(
        player.id,
        &player.name,
        player
            .party
            .into_iter()
            .map(BorrowedPokemon::Owned)
            .collect(),
        Box::new(NetBattleClientInto::from(player.client, sender, receiver, map)),
        1,
    )
}

// enum ServerState {
//     Connecting,
//     Battle,
// }

impl NetBattleClientInto {
    pub fn from(
        cli: NetBattleClient,
        sender: Sender<Packet>,
        receiver: Receiver<SocketEvent>,
        map: Rc<DashMap<SocketAddr, ClientMessage>>,
    ) -> Self {
        Self {
            sender,
            receiver,
            addr: cli.0,
            map,
        }
    }
}

pub struct NetBattleClientInto {
    receiver: Receiver<SocketEvent>,
    sender: Sender<Packet>,
    addr: SocketAddr,
    map: Rc<DashMap<SocketAddr, ClientMessage>>,
}

impl BattleEndpoint for NetBattleClientInto {
    fn give_client(&mut self, message: ServerMessage) {
        debug!("Sending message {:?} to client at {}", message, self.addr);
        self.sender
            .send(Packet::reliable_unordered(
                self.addr,
                ser::serialize(&message).unwrap(),
            ))
            .unwrap();
    }
}

impl BattleClient for NetBattleClientInto {
    fn give_server(&mut self) -> Option<ClientMessage> {
        if let Ok(event) = self.receiver.try_recv() {
            match event {
                SocketEvent::Packet(packet) => match ser::deserialize(packet.payload()) {
                    Ok(message) => {
                        debug!("Received message from {}: {:?}", self.addr, message);
                        self.map.insert(packet.addr(), message);
                    }
                    Err(err) => warn!(
                        "Could not deserialize client message from {} with error {}",
                        self.addr, err
                    ),
                },
                _ => (),
            }
        }
        self.map.remove(&self.addr).map(|(_, m)| m)
    }
}
