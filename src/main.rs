pub mod commands;
pub mod conf;
pub mod console;
pub mod downloader;
pub mod format;
pub mod player_state;
pub mod remote;
pub mod song;

use commands::PlayerMessage;

use format::{Format, Formattable};
use rodio::{OutputStream, Sink, Source};
use tokio::sync::mpsc::channel;

use std::collections::VecDeque;

use std::fs::read_dir;
use std::io::{stdin, BufRead};

use std::path::PathBuf;
use std::process::exit;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::*;

use crate::conf::Configuration;
use crate::console::handle_command;
use crate::player_state::PlayerState;
use crate::remote::RemoteHandler;
use crate::song::Song;

#[tokio::main]
async fn main() {
    println!("Starting player");
    let (ps, mut pr) = channel(128);
    let status: Arc<Mutex<PlayerState>> = Arc::new(Mutex::new(PlayerState {
        now_playing: None,
        queue: VecDeque::new(),
        volume: 1.0,
        speed: 1.0,
        paused: true,
        source_duration: None,
    }));
    let status_sender = status.clone();
    let conf = Configuration::get_conf();
    let mut remote_handler = RemoteHandler::new(ps.clone(), status.clone());

    std::thread::spawn(move || {
        let mut queue: VecDeque<Song> = VecDeque::new();
        let mut now_playing: Option<Song> = None;
        let mut current_duration: Option<Duration> = None;
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        sink.set_volume(conf.default_volume);

        loop {
            // Add the next song to the queue if the queue is empty
            if sink.empty() && !queue.is_empty() {
                let song = queue.pop_front();
                if let Some(song) = song {
                    let source = song.create_source();
                    match source {
                        Ok(source) => {
                            current_duration = source.total_duration();
                            now_playing = Some(song);
                            sink.append(source);
                        }
                        Err(e) => println!("Error reached when appending: {:#?}", e),
                    }
                }
            } else if sink.empty() && queue.is_empty() {
                now_playing = None;
                current_duration = None;
            }

            // Update state
            let mut editable = status_sender.lock().unwrap();
            editable.now_playing = now_playing.clone();
            editable.queue = queue.clone();
            editable.speed = sink.speed();
            editable.volume = sink.volume();
            editable.paused = sink.is_paused();
            editable.source_duration = current_duration;
            drop(editable);

            // Handle a message if one is recieved
            let message_or_error = pr.try_recv();
            if let Ok(message) = message_or_error {
                match message {
                    PlayerMessage::Stop => {
                        queue.clear();
                        sink.stop();
                    }
                    PlayerMessage::Pause => sink.pause(),
                    PlayerMessage::Play => sink.play(),
                    PlayerMessage::Volume(v) => sink.set_volume(v),
                    PlayerMessage::Skip(list) => {
                        let mut sorted = list.clone();
                        sorted.sort_by(|a, b| b.cmp(a));
                        for index in sorted.as_ref() {
                            match index {
                                0 => sink.stop(),
                                _ => {
                                    queue.remove(*index - 1);
                                }
                            }
                        }
                    }
                    PlayerMessage::Add(s) => {
                        queue.push_back(s);
                    }
                    PlayerMessage::Clear => queue.clear(),
                    PlayerMessage::Speed(s) => sink.set_speed(s),
                    PlayerMessage::ReOrder(origin, dest) => {
                        let elem = queue.remove(origin);
                        if let Some(song) = elem {
                            queue.insert(dest, song)
                        }
                    }
                }
            }
        }
    });

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
