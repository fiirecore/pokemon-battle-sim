extern crate firecore_battle_net as common;

use std::{cell::UnsafeCell, net::SocketAddr, rc::Rc};

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
        deps::{ser, hash::HashMap},
        log::{error, info, warn},
        pokedex::{
            moves::usage::script::engine,
            pokemon::instance::BorrowedPokemon,
        },
    },
    laminar::{Packet, Socket, SocketEvent},
    NetClientMessage, NetServerMessage, Player,
};

pub fn main() {

    common::init();

    let mut players = Vec::with_capacity(2);
    let mut battle = Battle::new(engine());

    let address = SocketAddr::new(common::ip().unwrap(), common::SERVER_PORT);

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

    let map = Rc::new(UnsafeCell::new(HashMap::new()));

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
        std::thread::sleep(std::time::Duration::from_millis(3));
    }

    info!("closing server.");
}

type SharedReceiver = Rc<UnsafeCell<HashMap<SocketAddr, ClientMessage>>>;

fn player(
    player: Player,
    sender: Sender<Packet>,
    receiver: Receiver<SocketEvent>,
    map: SharedReceiver,
) -> BattlePlayer {
    BattlePlayer::new(
        player.id,
        Some(player.trainer),
        player
            .party
            .into_iter()
            .map(BorrowedPokemon::Owned)
            .collect(),
        Box::new(NetBattleClientInto {
            addr: player.client.0, sender, receiver, map 
        }),
        1,
    )
}

pub struct NetBattleClientInto {
    addr: SocketAddr,
    receiver: Receiver<SocketEvent>,
    sender: Sender<Packet>,
    map: SharedReceiver,
}

impl BattleEndpoint for NetBattleClientInto {
    fn give_client(&mut self, message: ServerMessage) {
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
                        unsafe{self.map.get().as_mut().unwrap()}.insert(packet.addr(), message);
                    }
                    Err(err) => warn!(
                        "Could not deserialize client message from {} with error {}",
                        self.addr, err
                    ),
                },
                _ => (),
            }
        }
        unsafe{self.map.get().as_mut().unwrap()}.remove(&self.addr)
    }
}
