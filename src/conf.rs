use std::fs::read_to_string;
use std::result::Result::*;
use serde::{Deserialize, Serialize};

static CONF_PATH: &str = "conf.json";

#[derive(Serialize, Deserialize, Debug)]
pub struct Configuration {
    pub access_key: String,
    pub default_volume: f32,
    pub owned_path: String,
    pub outer_paths: Vec<String>,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration{
            access_key: "".to_string(),
            default_volume: 1.0,
            owned_path: "songs/".to_string(),
            outer_paths: Vec::new()
        }
    }
}

impl Configuration {
    pub fn new() -> Result<Configuration, Configuration> {
        match read_to_string(CONF_PATH){
            Ok(string) => {
                let conf = serde_json::from_str(&string);
                match conf {
                    Err(_) => Err(Configuration::default()),
                    Ok(c) => Ok(c)
                }
            }
            Err(_) => Err(Configuration::default())
        }
    }
    pub fn get_conf() -> Configuration {
    match Configuration::new(){
        Ok(c) => c,
        Err(c) => {
            println!("Could not find conf file, resorting to default");
            c
        }
    }
    }
}
