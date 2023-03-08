use std::{
    fs::File,
    io::{self, BufReader},
};

use rodio::{decoder::DecoderError, Decoder};
use serde::{Deserialize, Serialize};

use crate::SONG_PATH;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Song {
    pub name: String,
    pub artist: String,
    pub url: String,
}

impl Song {
    pub fn create_source(&self) -> Result<Decoder<BufReader<File>>, DecoderError> {
        let buf = self.create_buf();
        return match buf {
            Ok(buf) => Decoder::new(buf),
            Err(e) => Err(DecoderError::IoError(e.to_string())),
        };
    }

    fn create_buf(&self) -> io::Result<BufReader<File>> {
        let path = format!("{}{}.mp3", SONG_PATH, &self.name);
        let file = File::open(path)?;
        Ok(BufReader::new(file))
    }
}
