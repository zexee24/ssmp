use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::Receiver;

use rodio::{OutputStream, Sink, Source};
use tokio::time::Instant;

use crate::commands::PlayerMessage;
use crate::conf;
use crate::player_state::PlayerState;
use crate::song::Song;

pub async fn start_player(mut pr: Receiver<PlayerMessage>, status_sender: Arc<Mutex<PlayerState>>) {
    tokio::spawn(async move {
        let conf = conf::Configuration::get_conf();
        let mut queue: VecDeque<Song> = VecDeque::new();
        let mut now_playing: Option<Song> = None;
        let mut current_duration: Option<Duration> = None;
        let mut total_duration: Option<Duration> = None;
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        sink.set_volume(conf.default_volume);
        let mut t = Instant::now();
        loop {
            // Add the next song to the queue if the queue is empty
            if sink.empty() && !queue.is_empty() {
                let song = queue.pop_front();
                if let Some(song) = song {
                    let source = song.create_source();
                    match source {
                        Ok(source) => {
                            total_duration = mp3_duration::from_path(&song.path).ok();
                            now_playing = Some(song);
                            sink.append(source);
                            t = Instant::now();
                        }
                        Err(e) => println!("Error reached when appending: {:#?}", e),
                    }
                }
            } else if sink.empty() && queue.is_empty() {
                now_playing = None;
                current_duration = None;
                total_duration = None;
            }
            if now_playing.is_some() && !sink.is_paused() {
                current_duration = Some(t.elapsed().mul_f32(sink.speed()));
            }

            // Update state
            let mut editable = status_sender.lock().unwrap();
            editable.now_playing = now_playing.clone();
            editable.queue = queue.clone();
            editable.speed = sink.speed();
            editable.volume = sink.volume();
            editable.paused = sink.is_paused();
            editable.total_duration = total_duration;
            editable.current_duration = current_duration;
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
                    PlayerMessage::Play => {
                        sink.play();
                        t=Instant::now().checked_sub(current_duration.unwrap_or(Duration::from_secs(0))).unwrap();
                    },
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
                    PlayerMessage::Speed(s) => {
                        t = Instant::now().checked_sub(current_duration.unwrap_or(Duration::new(0,0).mul_f32(s))).unwrap();
                        sink.set_speed(s);
                    },
                    PlayerMessage::ReOrder(origin, mut dest) => {
                        let elem = queue.remove(origin);
                        if let Some(song) = elem {
                            if dest >= origin {
                                dest -= 1;
                            }
                            queue.insert(dest.min(queue.len()), song)
                        }
                    }
                    PlayerMessage::Seek(n) => {
                        sink.stop();
                        if let Some(song) = &now_playing {
                            match song.create_source() {
                                Ok(s) => {
                                    let dur = Duration::from_secs(n);
                                    sink.append(s.skip_duration(dur));
                                    t = Instant::now().checked_sub(dur).unwrap();
                                },
                                Err(e) => println!("Failed seek because {:?}", e),
                            }
                        }
                    }
                }
            }
        }
    });
}
