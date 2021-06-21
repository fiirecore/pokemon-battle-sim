fn main() {
    winres::WindowsResource::new()
        .set_icon("../icon.ico")
        .compile().unwrap();
}