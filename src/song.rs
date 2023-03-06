use std::{ffi::OsString, io::{BufReader, self}, fs::File};

use rodio::{Decoder, decoder::DecoderError};
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Song {
    pub name : String,
    pub artist : String,
    pub url : String,
    pub path : OsString
}

impl Song {
    pub fn create_source(&self) -> Result<Decoder<BufReader<File>>, DecoderError> {
        let buf = self.create_buf();
        return match buf {
            Ok(buf) => Decoder::new(buf),
            Err(e) => Err(DecoderError::IoError(e.to_string()))
        }
    }

    fn create_buf(&self) -> io::Result<BufReader<File>>{
        let file = File::open(&self.path)?;
        Ok(BufReader::new(file))
    }
}