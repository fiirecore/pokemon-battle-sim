fn main() {
    dex_builder_mini::compile_from_normal(
        dex_builder_mini::deserialize_normal("../client/dex.bin"),
        "dex.bin",
    );
    #[cfg(windows)]
    winres::WindowsResource::new()
        .set_icon("../icon.ico")
        .compile()
        .unwrap();
}
