fn main() {
    dex_builder::compile("../assets/pokedex/pokemon", "../assets/pokedex/moves", "../assets/pokedex/items", "../assets/pokedex/trainers", Some("dex.bin"), true);
    font_builder::compile("../assets/fonts", "fonts.bin");
    #[cfg(windows)]
    winres::WindowsResource::new()
        .set_icon("../icon.ico")
        .compile().unwrap();
}