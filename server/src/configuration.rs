use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{
    fs::{read_to_string, write},
    path::Path,
};

#[derive(Deserialize, Serialize)]
pub struct Configuration {
    pub port: u16,
    pub battle_size: u8,
    // pub ai: u8,
}

impl Configuration {
    const FILENAME: &'static str = "config.toml";

    pub fn load() -> Self {
        let path = match std::env::current_exe() {
            Ok(ref path) => path
                .parent()
                .unwrap_or_else(|| panic!("Could not get parent directory of executable!")),
            Err(err) => {
                error!(
                    "Could not get path of current executable with error {}\nUsing fallback path.",
                    err
                );
                Path::new(".")
            }
        }
        .join(Self::FILENAME);
        match read_to_string(&path) {
            Ok(bytes) => toml::from_str(&bytes).unwrap_or_else(|err| {
                panic!("Could not deserialize configuration with error: {}", err)
            }),
            Err(err) => {
                error!(
                    "Could not read configuration file at {:?} with error {}",
                    path, err
                );
                let configuration = Configuration::default();
                write(
                    &path,
                    toml::to_string(&configuration).unwrap_or_else(|err| {
                        panic!(
                            "Could not serialize configuration with error {} \nThis is bad.",
                            err
                        )
                    }),
                )
                .unwrap_or_else(|err| {
                    panic!(
                        "Could not write to configuration file at {:?} with error {}",
                        path, err
                    )
                });
                info!("Created a new configuration file.");
                configuration
            }
        }
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            port: common::DEFAULT_PORT,
            battle_size: 1,
            // ai: 0,
        }
    }
}
