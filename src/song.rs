use std::{
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
};

use id3::{Tag, TagLike};
use rodio::{decoder::DecoderError, Decoder};
use serde::{Deserialize, Serialize};

use crate::format::Formattable;
use crate::{format::Format, list_songs};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Song {
    pub name: String,
    pub artist: Option<String>,
    pub url: Option<String>,
    #[serde(skip)]
    pub path: PathBuf,
    pub format: Format,
}

impl Song {
    pub fn create_source(&self) -> Result<Decoder<BufReader<File>>, DecoderError> {
        let buf = self.create_buf();
        match buf {
            Ok(buf) => Decoder::new(buf),
            Err(e) => Err(DecoderError::IoError(e.to_string())),
        }
    }

    fn create_buf(&self) -> io::Result<BufReader<File>> {
        let file = File::open(&self.path)?;
        Ok(BufReader::new(file))
    }

    pub fn from_file(path: PathBuf) -> Option<Song> {
        let tag = Tag::read_from_path(&path).unwrap_or(Tag::new());
        let url = tag
            .extended_texts()
            .find(|item| item.description == *"url")
            .map(|url| url.value.as_str());
        let filename = path
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        Some(Song {
            name: tag.title().unwrap_or(filename).to_string(),
            artist: tag.artist().map(|s| s.to_string()),
            url: url.map(|s| s.to_string()),
            path: path.clone(),
            format: path.get_format(),
        })
    }

    pub fn from_string(string: String) -> Option<Song> {
        list_songs()
            .into_iter()
            .find(|song| song.name == string.clone() || song.url == Some(string.clone()))
    }
}

impl Default for Song {
    fn default() -> Self {
        Song {
            name: "Unknown name".to_string(),
            artist: None,
            url: None,
            path: PathBuf::new(),
            format: Format::UNSUPPORTED,
        }
    }
}
