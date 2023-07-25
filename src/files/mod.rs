use std::fs::read_dir;
use std::path::PathBuf;

use crate::conf::Configuration;
use crate::format::{Format, Formattable};
use crate::song::Song;

//TODO make this async
pub fn list_songs() -> Vec<Song> {
    let mut song_list: Vec<Song> = Vec::new();
    let conf = Configuration::get_conf();
    let owned_path = conf.owned_path;
    let outer_paths = conf.outer_paths;
    let mut total_path = outer_paths.to_vec();
    total_path.push(owned_path);

    for dir_str in total_path {
        song_list.append(&mut scan_folder(dir_str))
    }
    song_list
}

fn scan_folder(folder: PathBuf) -> Vec<Song> {
    let mut song_vec = Vec::new();
    if let Ok(dir) = read_dir(folder) {
        for entry in dir.flatten() {
            if entry.get_format() != Format::UNSUPPORTED {
                if let Some(song) = Song::from_file(entry.path()) {
                    song_vec.push(song)
                }
            } else if let Ok(filetype) = entry.file_type() {
                if filetype.is_dir() {
                    song_vec.append(&mut scan_folder(entry.path()))
                }
            }
        }
    }
    song_vec
}
