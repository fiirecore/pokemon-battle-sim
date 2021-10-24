use std::path::Path;

use firecore_battle_builder::compile as build_battle;

fn main() {

    println!("cargo:rerun-if-changed=../assets");

    let battle = Path::new("../assets/pokedex/battle");

    let battle = build_battle(battle, &battle.join("scripts"));

    let data = bincode::serialize(&battle)
    .unwrap_or_else(|err| panic!("Could not serialize battle move binary with error {}", err));

    let output = "battle.bin";
    
    std::fs::write(output, &data).unwrap_or_else(|err| {
        panic!(
            "Cannot create / write to battle binary file at {:?} with error {}",
            output, err
        )
    });
}
