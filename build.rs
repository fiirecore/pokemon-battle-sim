fn main() {
    #[cfg(feature = "build")] {

        use std::path::Path;
        use firecore_pokedex_builder::compile as build_pokedex;

        let input = Path::new("assets/pokedex");

        let dex = build_pokedex(
            input.join("pokemon"),
            input.join("moves"),
            input.join("items"),
        );

        let data = bincode::serialize(&dex)
        .unwrap_or_else(|err| panic!("Could not serialize dex binary with error {}", err));

        let output = "dex.bin";

        std::fs::write(output, &data).unwrap_or_else(|err| {
            panic!(
                "Cannot create / write to dex binary file at {:?} with error {}",
                output, err
            )
        });

        #[cfg(windows)]
        winres::WindowsResource::new()
            .set_icon("icon.ico")
            .compile()
            .unwrap();   
    }
}