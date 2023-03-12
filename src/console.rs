use std::sync::atomic::Ordering::SeqCst;
use std::sync::{atomic::AtomicBool, mpsc::Sender, Arc, Mutex};
use std::thread;

use crate::conf::Configuration;
use crate::downloader;
use crate::remote::start_remote;
use crate::song::Song;
use crate::{commands::PlayerMessage, exit_program, list_songs, player_state::PlayerState};

pub(crate) fn handle_command(
    command: &str,
    ps: Sender<PlayerMessage>,
    stop_remote: Arc<AtomicBool>,
    state: Arc<Mutex<PlayerState>>,
) {
    let (command, value) = command.split_once(' ').unwrap_or((command, ""));
    match command {
        "list" | "ls" => {
            for song in list_songs() {
                println!("{}", song.name)
            }
        }
        "volume" => ps
            .send(PlayerMessage::Volume(value.parse::<f32>().unwrap_or(1.0)))
            .unwrap(),
        "add" => {
            let songopt = Song::from_string(value.to_owned());
            if let Some(song) = songopt {
                ps.send(PlayerMessage::Add(song)).unwrap();
            }
        }
        "play" | "continue" | "p" => ps.send(PlayerMessage::Play).unwrap(),
        "stop" => ps.send(PlayerMessage::Stop).unwrap(),
        "clear" => ps.send(PlayerMessage::Clear).unwrap(),
        "pause" => ps.send(PlayerMessage::Pause).unwrap(),
        "exit" => exit_program(),
        "speed" => ps
            .send(PlayerMessage::Speed((value.parse::<f32>()).unwrap_or(1.0)))
            .unwrap(),
        "remote" => match value {
            "start" => {
                let addrs: Vec<String> = Configuration::get_conf().ip;
                start_remote(ps, stop_remote.clone(), state, addrs);
                stop_remote.store(false, SeqCst)
            }
            "stop" => stop_remote.store(true, SeqCst),
            _ => println!("Unknown subcommand of'remote'"),
        },
        "now" | "nowplaying" | "current" | "np" => {
            println!("{:?}", state.lock().unwrap().now_playing)
        }
        "queue" | "que" => {
            let queue = &state.lock().unwrap().queue;
            for song in queue {
                println!("{}", song.name)
            }
        }
        "status" => {
            println!("{:#?}", state.lock().unwrap())
        }
        "move" | "reorder" => {
            let t = value.split_once(' ').unwrap_or(("0", "0"));
            let (from, to) = (
                t.0.parse::<usize>().unwrap_or(0),
                t.1.parse::<usize>().unwrap_or(0),
            );
            ps.send(PlayerMessage::ReOrder(from, to)).unwrap();
        }
        "skip" => {
            let mut list: Vec<usize> = Vec::new();
            for arg in value.split(' ') {
                let num = arg.parse::<usize>();
                if let Ok(num) = num {
                    list.push(num)
                }
            }
            //Default behaviour
            if list.is_empty() {
                list.push(0)
            }
            ps.send(PlayerMessage::Skip(list.into())).unwrap();
        }
        "download" | "d" => {
            let val = (*value).to_string();
            thread::spawn(|| {
                let result = downloader::download(val);
                if let Err(e) = result {
                    println!("{e}");
                }
            });
        }
        "download-add" | "da" => {
            let val = (*value).to_string();
            thread::spawn(move || {
                let result = downloader::download(val);
                match result {
                    Err(e) => println!("{e}"),
                    Ok(song) => ps.send(PlayerMessage::Add(song)).unwrap(),
                }
            });
        }
        _ => println!("Unknown command"),
    }
}
