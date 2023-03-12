use std::{
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
};

use id3::{frame::PictureType, Tag, TagLike};
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
        let url_frame = tag.get("WOAF");
        let filename = path
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        let url = match url_frame {
            Some(frame) => frame.content().link().map(|s| s.to_string()),
            None => None,
        };
        Some(Song {
            name: tag.title().unwrap_or(filename).to_string(),
            artist: tag.artist().map(|s| s.to_string()),
            url,
            path: path.clone(),
            format: path.get_format(),
        })
    }

    pub fn from_string(string: String) -> Option<Song> {
        list_songs()
            .into_iter()
            .find(|song| song.name == string.clone() || song.url == Some(string.clone()))
    }

    pub fn get_image(&self) -> Option<Vec<u8>> {
        let tag = Tag::read_from_path(&self.path).ok()?;
        let image_data = tag
            .pictures()
            .find(|p| p.picture_type == PictureType::CoverFront)?;
        Some(image_data.data.clone())
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
