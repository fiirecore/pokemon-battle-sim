fn main() {
    winres::WindowsResource::new()
        .set_icon("../../pokemon-game/build/icon.ico")
        .compile().unwrap();
}