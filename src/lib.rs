pub extern crate firecore_game as game;
pub extern crate simple_logger as logger;
pub extern crate laminar;

pub static DEX_BYTES: &[u8] = include_bytes!("../../pokemon-game/build/data/dex.bin");

pub const SERVER_PORT: u16 = 14191;

use std::net::SocketAddr;
use game::pokedex::{
    pokemon::party::PokemonParty,
    moves::target::PlayerId,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub enum NetClientMessage {
    RequestConnect,
    Connect(Player)
}

#[derive(Deserialize, Serialize)]
pub enum NetServerMessage {
    AcceptConnect,
    // Begin,
}

#[derive(Deserialize, Serialize)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub party: PokemonParty,
    pub client: NetBattleClient,
}

#[derive(Deserialize, Serialize)]
pub struct NetBattleClient(pub SocketAddr);

pub fn init() {
    simple_logger::SimpleLogger::new().init().unwrap();
    pokedex_no_ctx(game::deps::ser::deserialize(DEX_BYTES).unwrap());
}

pub fn pokedex_no_ctx(dex: game::pokedex::serialize::SerializedDex) {

    use game::{
        deps::{self, hash::HashMap},
        pokedex,
    };

    let mut pokedex = HashMap::with_capacity(dex.pokemon.len());

    pokedex.insert(
        <pokedex::pokemon::Pokemon as deps::borrow::Identifiable>::UNKNOWN, 
        pokedex::pokemon::Pokemon {
            id: <pokedex::pokemon::Pokemon as deps::borrow::Identifiable>::UNKNOWN,
            name: "Unknown".to_string(),
            primary_type: pokedex::types::PokemonType::default(),
            secondary_type: None,
            base: Default::default(),
            data: pokedex::pokemon::data::PokedexData {
                species: "Unknown".to_string(),
                height: 0,
                weight: 0,
            },
            training: pokedex::pokemon::data::Training {
                base_exp: 0,
                growth_rate: Default::default(),
            },
            breeding: pokedex::pokemon::data::Breeding {
                gender: None,
            },
            moves: Vec::new(),
        }
    );

	for pokemon in dex.pokemon {	
		pokedex.insert(pokemon.pokemon.id, pokemon.pokemon);
	}

    pokedex::pokemon::dex::set(pokedex);

	let mut movedex = HashMap::with_capacity(dex.moves.len());

	for serialized_move in dex.moves {
        let pmove = serialized_move.pokemon_move;
		movedex.insert(pmove.id, pmove);
	}

    pokedex::moves::dex::set(movedex);

    let mut itemdex = HashMap::with_capacity(dex.items.len());

    for item in dex.items {
        itemdex.insert(item.item.id, item.item);
    }

    pokedex::item::dex::set(itemdex);

}

// 

// fn main() -> Result {
//     match std::env::args().skip(1).next() {
//         Some(t) => match t.trim_end() {
//             "c" | "client" => client::main(),
//             "s" | "server" => Ok(server::main()),
//             _ => {
//                 info!("Could not recognize argument {}", t);
//                 Ok(())
//             },
//         },
//         None => {
//             info!("Could not run server / client because neither was specified");
//             Ok(())
//         }
//     }
    
//     // client::main()
// }