use std::{collections::BTreeMap, fs::write, path::Path};

use firecore_pokedex_game::{
    item::{Item, ItemId},
    moves::{Move, MoveId},
    pokemon::{Pokemon, PokemonId},
    serialize::SerializedDex,
};

pub fn deserialize_normal(path: impl AsRef<Path>) -> SerializedDex {
    let path = path.as_ref();
    firecore_dependencies::ser::deserialize(&std::fs::read(path).unwrap_or_else(|err| {
        panic!(
            "Could not read SerializedDex file at {:?} with error {}",
            path, err
        )
    }))
    .unwrap_or_else(|err| panic!("Could not deserialize SerializedDex with error {}", err))
}

pub fn compile_from_normal(dex: SerializedDex, output: impl AsRef<Path>) {
    let output = output.as_ref();

    let data = (
        dex.pokemon
            .into_iter()
            .map(|p| (p.pokemon.id, p.pokemon))
            .collect::<BTreeMap<PokemonId, Pokemon>>(),
        dex.moves
            .into_iter()
            .map(|m| (m.pokemon_move.id, m.pokemon_move))
            .collect::<BTreeMap<MoveId, Move>>(),
        dex.items
            .into_iter()
            .map(|i| (i.item.id, i.item))
            .collect::<BTreeMap<ItemId, Item>>(),
    );
    let data = firecore_dependencies::ser::serialize(&data)
        .unwrap_or_else(|err| panic!("Could not serialize mini dex binary with error {}", err));

    write(output, &data).unwrap_or_else(|err| {
        panic!(
            "Cannot create / write to output file at {:?} with error {}",
            output, err
        )
    });
}
