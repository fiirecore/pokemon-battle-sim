fn main() {
    dex_builder_mini::compile(
        dex_builder::compile("../assets/pokedex/pokemon", "../assets/pokedex/moves", "../assets/pokedex/items", "../assets/pokedex/trainers", None, false),
        "dex.bin",
    );
    #[cfg(windows)]
    winres::WindowsResource::new()
        .set_icon("../icon.ico")
        .compile()
        .unwrap();
}
