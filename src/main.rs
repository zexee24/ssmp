pub mod commands;
pub mod conf;
pub mod console;
pub mod downloader;
pub mod format;
mod player;
pub mod player_state;
pub mod remote;
pub mod song;

use format::{Format, Formattable};

use tokio::sync::mpsc::channel;

use std::collections::VecDeque;

use std::fs::read_dir;
use std::io::{stdin, BufRead};

use std::path::PathBuf;
use std::process::exit;

use std::sync::{Arc, Mutex};

use std::*;

use crate::conf::Configuration;
use crate::console::handle_command;
use crate::player::start_player;
use crate::player_state::PlayerState;
use crate::remote::RemoteHandler;
use crate::song::Song;

#[tokio::main]
async fn main() {
    println!("Starting player");
    let (ps, pr) = channel(128);
    let status: Arc<Mutex<PlayerState>> = Arc::new(Mutex::new(PlayerState {
        now_playing: None,
        queue: VecDeque::new(),
        volume: 1.0,
        speed: 1.0,
        paused: true,
        total_duration: None,
        current_duration: None,
    }));
    let mut remote_handler = RemoteHandler::new(ps.clone(), status.clone());
    start_player(pr, status.clone()).await;

    for command in stdin().lock().lines() {
        match command {
            Ok(command) => {
                handle_command(
                    command.trim(),
                    ps.clone(),
                    status.clone(),
                    &mut remote_handler,
                )
                .await
            }
            Err(_) => println!("Error handling input stream"),
        }
    }
    exit_program()
}

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

fn exit_program() {
    println!("Exitting");
    exit(0)
}
