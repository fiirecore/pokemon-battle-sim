fn main() {
    firecore_pokedex_builder::compile(
        "../assets/pokedex/pokemon",
        "../assets/pokedex/moves",
        "../assets/pokedex/items",
        "../assets/pokedex/trainers",
        Some("dex.bin"),
        cfg!(feature = "audio"),
    );
    firecore_font_builder::compile("../assets/fonts", "fonts.bin");
    #[cfg(windows)]
    winres::WindowsResource::new()
        .set_icon("../icon.ico")
        .compile()
        .unwrap();
}
