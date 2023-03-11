use std::{fs::DirEntry, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Format {
    MP3,
    MP4,
    UNSUPPORTED,
}
static FORMAT_MAP: &[(&str, Format)] = &[(".mp3", Format::MP3), (".mp4", Format::MP4)];

impl Format {
    pub fn extension_to_filetype(extension: &str) -> Format {
        for (ex, fr) in FORMAT_MAP.iter() {
            if ex == &extension {
                return fr.clone();
            }
        }
        Format::UNSUPPORTED
    }
    pub fn filetype_to_extension(format: Format) -> Option<String> {
        for (ex, fr) in FORMAT_MAP.iter() {
            if &format == fr {
                return Some(ex.to_string());
            }
        }
        None
    }
}

pub(crate) trait Formattable {
    fn get_format(&self) -> Format;
}

impl Formattable for DirEntry {
    fn get_format(&self) -> Format {
        if let Ok(file_name) = self.file_name().into_string() {
            let extension = ".".to_owned() + file_name.split('.').last().unwrap_or("");
            return Format::extension_to_filetype(extension.as_str());
        }
        Format::UNSUPPORTED
    }
}

impl Formattable for PathBuf {
    fn get_format(&self) -> Format {
        if let Some(extension) = &self.extension() {
            return Format::extension_to_filetype(extension.to_str().unwrap_or_default());
        }
        Format::UNSUPPORTED
    }
}
