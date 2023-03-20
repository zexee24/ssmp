use serde::{Deserialize, Serialize};

use crate::remote::auth::Key;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::result::Result::*;
use std::str::FromStr;

static CONF_PATH: &str = "conf.json";

#[derive(Serialize, Deserialize, Debug)]
pub struct Configuration {
    #[serde()]
    pub keys: Vec<Key>,
    pub default_volume: f32,
    pub owned_path: PathBuf,
    pub outer_paths: Vec<PathBuf>,
    pub ip: Vec<String>,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            keys: vec![Key::default()],
            default_volume: 1.0,
            owned_path: PathBuf::from_str("songs/").unwrap(),
            outer_paths: Vec::new(),
            ip: vec!["0.0.0.0:8000".to_string(), "127.0.0.1:8000".to_string()],
        }
    }
}

impl Configuration {
    fn new() -> Result<Configuration, Configuration> {
        match read_to_string(CONF_PATH) {
            Ok(string) => {
                let conf: Result<Configuration, serde_json::Error> = serde_json::from_str(&string);
                match conf {
                    Err(_) => Err(Configuration::default()),
                    Ok(mut c) => {
                        for key in &mut c.keys {
                            key.convert_all();
                        }
                        Ok(c)
                    }
                }
            }
            Err(_) => Err(Configuration::default()),
        }
    }
    pub fn get_conf() -> Configuration {
        match Configuration::new() {
            Ok(c) => c,
            Err(c) => {
                println!("Could not find conf file, resorting to default");
                c
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::conf::Configuration;

    #[test]
    fn test_reading_conf() {
        assert!(Configuration::new().is_ok());
    }
}
