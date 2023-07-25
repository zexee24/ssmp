/*use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::Sender;

use crate::conf::Configuration;
use crate::downloader;
use crate::remote::RemoteHandler;
use crate::song::Song;
use crate::{commands::PlayerMessage, exit_program, list_songs, player_state::PlayerState};

pub(crate) async fn handle_command(
    command: &str,
    ps: Sender<PlayerMessage>,
    state: Arc<Mutex<PlayerState>>,
    remote_handler: &mut RemoteHandler,
) {
    let conf = Configuration::get_conf();
    let (command, value) = command.split_once(' ').unwrap_or((command, ""));
    match command {
        "list" | "ls" => {
            for song in list_songs() {
                println!("{}", song.name)
            }
        }
        "volume" => ps
            .send(PlayerMessage::Volume(value.parse::<f32>().unwrap_or(1.0)))
            .await
            .unwrap(),
        "add" => {
            let songopt = Song::from_string(value.to_owned());
            if let Some(song) = songopt {
                ps.send(PlayerMessage::Add(song)).await.unwrap();
            }
        }
        "play" | "continue" | "p" => ps.send(PlayerMessage::Play).await.unwrap(),
        "stop" => ps.send(PlayerMessage::Stop).await.unwrap(),
        "clear" => ps.send(PlayerMessage::Clear).await.unwrap(),
        "pause" => ps.send(PlayerMessage::Pause).await.unwrap(),
        "exit" => exit_program(),
        "speed" => ps
            .send(PlayerMessage::Speed((value.parse::<f32>()).unwrap_or(1.0)))
            .await
            .unwrap(),
        "remote" => {
            let (c, v1) = value.split_once(' ').unwrap_or((value, ""));
            match c {
                "start" => {
                    for addr in v1.split(' ') {
                        match addr {
                            "default" => {
                                for addr in &conf.ip {
                                    match remote_handler.new_listener(addr.to_owned()).await {
                                        Ok(_) => {
                                            println!("Successfully started remote on {}", addr)
                                        }
                                        Err(e) => println!(
                                            "Failed to start remote on {} because {}",
                                            addr, e
                                        ),
                                    }
                                }
                            }
                            _ => match remote_handler.new_listener(addr.to_owned()).await {
                                Ok(_) => println!("Successfully started remote on {}", addr),
                                Err(e) => {
                                    println!("Failed to start remote on {} because {}", addr, e)
                                }
                            },
                        }
                    }
                }
                "stop" => {
                    for addr in v1.split(' ') {
                        match addr {
                            "default" => {
                                for addr in &conf.ip {
                                    match remote_handler.stop_listener(addr.to_owned()) {
                                        Ok(_) => {
                                            println!("Successfully stopped remote on {}", addr)
                                        }
                                        Err(e) => println!(
                                            "Failed to stop remote on {} because {}",
                                            addr, e
                                        ),
                                    }
                                }
                            }
                            "all" => {
                                for listener in remote_handler.list_listeners() {
                                    match remote_handler.stop_listener(listener.to_owned()) {
                                        Ok(_) => {
                                            println!("Successfully stopped remote on {}", listener)
                                        }
                                        Err(e) => {
                                            println!(
                                                "Failed to stop remote on {} because {}",
                                                listener, e
                                            )
                                        }
                                    }
                                }
                            }
                            _ => match remote_handler.stop_listener(addr.trim().to_owned()) {
                                Ok(_) => println!("Successfully stopped remote on {}", addr),
                                Err(e) => {
                                    println!("Failed to stop remote on {} because {}", addr, e)
                                }
                            },
                        }
                    }
                }
                _ => {
                    println!("Unknown subcommand of remote")
                }
            }
        }
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
            ps.send(PlayerMessage::ReOrder(from, to)).await.unwrap();
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
            ps.send(PlayerMessage::Skip(list.into())).await.unwrap();
        }
        "seek" => match value.parse::<u64>() {
            Ok(t) => ps.send(PlayerMessage::Seek(t)).await.unwrap(),
            Err(e) => println!("Input a valid integer {:?}", e),
        },
        "download" | "d" => {
            let val = (*value).to_string();
            tokio::spawn(async {
                let result = downloader::download_dlp(val).await;
                if let Err(e) = result {
                    println!("{e}");
                }
            });
        }
        "download-add" | "da" => {
            let val = (*value).to_string();
            tokio::spawn(async move {
                let result = downloader::download_dlp(val).await;
                match result {
                    Err(e) => println!("{e}"),
                    Ok(song) => ps.send(PlayerMessage::Add(song)).await.unwrap(),
                }
            });
        }
        _ => println!("Unknown command"),
    }
}*/
