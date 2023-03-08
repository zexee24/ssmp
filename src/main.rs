pub mod commands;
pub mod console;
pub mod downloader;
pub mod player_state;
pub mod remote;
pub mod song;

use commands::PlayerMessage;

use rodio::{OutputStream, Sink, Source};
use serde_json::Value;

use std::collections::VecDeque;

use std::io::{stdin, BufRead};

use std::process::exit;
use std::sync::atomic::AtomicBool;

use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use std::*;

use crate::console::handle_command;
use crate::player_state::PlayerState;
use crate::song::Song;

static CONF_PATH: &str = "conf.json";

// Path (relative or absolute) to the folder
static SONG_PATH: &str = "songs/";

fn main() {
    println!("Starting player");
    let (ps, pr) = mpsc::channel::<commands::PlayerMessage>();
    let status: Arc<Mutex<PlayerState>> = Arc::new(Mutex::new(PlayerState {
        now_playing: None,
        queue: VecDeque::new(),
        volume: 1.0,
        speed: 1.0,
        paused: true,
        source_duration: None,
    }));
    let status_sender = status.clone();
    let stop_remote: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    std::thread::spawn(move || {
        let mut queue: VecDeque<Song> = VecDeque::new();
        let mut now_playing: Option<Song> = None;
        let mut current_duration: Option<Duration> = None;
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        let default_volume = match fs::read_to_string(CONF_PATH) {
            Ok(file) => match serde_json::from_str::<Value>(&file) {
                Ok(json) => json["Default-Volume"].as_f64().unwrap_or(1.0) as f32,
                Err(e) => {
                    println!("Failed to get default for {:?}", e);
                    1.0
                }
            },
            Err(_) => 1.0,
        };
        sink.set_volume(default_volume);

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
                        let song = Song {
                            name: s,
                            artist: "Artist unknown".to_string(),
                            url: "Url unknown".to_string(),
                        };
                        queue.push_back(song);
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
            Ok(command) => handle_command(
                command.trim(),
                ps.clone(),
                stop_remote.clone(),
                status.clone(),
            ),
            Err(_) => println!("Error handling input stream"),
        }
    }
    exit_program()
}

fn list_songs() -> Vec<String> {
    let mut song_list: Vec<String> = Vec::new();
    let dir = fs::read_dir(SONG_PATH).unwrap();
    for file in dir {
        let file = file.unwrap().file_name();
        if let Ok(s) = file.into_string() {
            if let Some((name, _)) = s.split_once('.') {
                song_list.push(name.to_string());
            }
        }
    }
    song_list
}

fn exit_program() {
    println!("Exitting");
    exit(0)
}
