fn main() {
    #[cfg(windows)]
    winres::WindowsResource::new()
        .set_icon("../icon.ico")
        .compile().unwrap();
}