use std::{fs::DirEntry, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Format {
    MP3,
    MP4,
    UNSUPPORTED,
}

impl Format {
    pub fn match_filetype(extension: &str) -> Format {
        match extension {
            "mp3" => Self::MP3,
            "mp4" => Self::MP4,
            _ => Self::UNSUPPORTED,
        }
    }
}

pub(crate) trait Formattable {
    fn get_format(&self) -> Format;
}

impl Formattable for DirEntry {
    fn get_format(&self) -> Format {
        if let Ok(file_name) = self.file_name().into_string() {
            let extension = file_name.split('.').last().unwrap_or("");
            return Format::match_filetype(extension);
        }
        Format::UNSUPPORTED
    }
}

impl Formattable for PathBuf {
    fn get_format(&self) -> Format {
        if let Some(extension) = &self.extension() {
            return Format::match_filetype(extension.to_str().unwrap_or_default());
        }
        Format::UNSUPPORTED
    }
}
