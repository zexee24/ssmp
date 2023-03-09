use std::{
    fs::{DirEntry, File, FileType},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Format {
    MP3,
    MP4,
    UNSUPPORTED,
}

impl Format {
    pub fn match_filetype(ft: FileType) -> Format {
        match format!("{:?}", ft).as_str() {
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
            match extension {
                "mp3" => return Format::MP3,
                "mp4" => return Format::MP4,
                _ => return Format::UNSUPPORTED,
            }
        }
        Format::UNSUPPORTED
    }
}

impl Formattable for PathBuf {
    fn get_format(&self) -> Format {
        if let Ok(file) = File::open(self) {
            if let Ok(metadata) = file.metadata() {
                return Format::match_filetype(metadata.file_type());
            }
            return Format::UNSUPPORTED;
        }
        Format::UNSUPPORTED
    }
}
