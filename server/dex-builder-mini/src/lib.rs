use std::{fs::write, path::Path};

use firecore_pokedex_game::{
    item::Item,
    moves::Move,
    pokemon::Pokemon,
    serialize::SerializedDex,
};

pub fn deserialize_from_path(path: impl AsRef<Path>) -> SerializedDex {
    let path = path.as_ref();
    firecore_dependencies::ser::deserialize(&std::fs::read(path).unwrap_or_else(|err| {
        panic!(
            "Could not read SerializedDex file at {:?} with error {}",
            path, err
        )
    }))
    .unwrap_or_else(|err| panic!("Could not deserialize SerializedDex with error {}", err))
}

pub fn compile(dex: SerializedDex, output: impl AsRef<Path>) {
    let output = output.as_ref();

    let mut data = (
        dex.pokemon
            .into_iter()
            .map(|p| p.pokemon)
            .collect::<Vec<Pokemon>>(),
        dex.moves
            .into_iter()
            .map(|m| m.pokemon_move)
            .collect::<Vec<Move>>(),
        dex.items.into_iter().map(|i| i.item).collect::<Vec<Item>>(),
    );

    data.0.sort_by_key(|p| p.id);
    data.1.sort_by_key(|m| m.id);
    data.2.sort_by_key(|i| i.id);

    let data = firecore_dependencies::ser::serialize(&data)
        .unwrap_or_else(|err| panic!("Could not serialize mini dex binary with error {}", err));

    write(output, &data).unwrap_or_else(|err| {
        panic!(
            "Cannot create / write to output file at {:?} with error {}",
            output, err
        )
    });
}
