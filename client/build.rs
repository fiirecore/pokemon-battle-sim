fn main() {
    write(
        &firecore_pokedex_engine_builder::compile(
            "../assets/pokedex/client/pokemon",
            "../assets/pokedex/client/items",
            "../assets/pokedex/client/trainers",
        ),
        "dex-engine.bin",
    );

    write(
        &firecore_font_builder::compile("../assets/fonts"),
        "fonts.bin",
    );

    fn write<D: serde::Serialize>(data: &D, file: impl AsRef<std::path::Path>) {
        std::fs::write(file, bincode::serialize(data).unwrap()).unwrap()
    }
}
