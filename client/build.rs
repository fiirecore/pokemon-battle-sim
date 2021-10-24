fn main() {

    let dex_engine = firecore_pokedex_engine_builder::compile(
        "../assets/pokedex/client/pokemon",
        "../assets/pokedex/client/items",
        "../assets/pokedex/client/trainers",
    );

    let data = bincode::serialize(&dex_engine)
        .unwrap_or_else(|err| panic!("Could not serialize dex engine binary with error {}", err));

    let output = "dex-engine.bin";

    std::fs::write(output, &data).unwrap_or_else(|err| {
        panic!(
            "Cannot create / write to dex file at {} with error {}",
            output, err
        )
    });

    firecore_font_builder::compile("../assets/fonts", "fonts.bin");
    
}
