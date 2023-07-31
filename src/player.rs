use relm4::Worker;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Duration;

use rodio::{OutputStream, Sink, Source};
use tokio::time::Instant;

use crate::commands::PlayerMessage;
use crate::player_state::PlayerState;
use crate::MainMessage;

pub(crate) struct Player {
    sender: Sender<PlayerMessage>,
}

impl Worker for Player {
    type Init = ();

    type Input = PlayerMessage;

    type Output = MainMessage;

    fn init(_init: Self::Init, sender: relm4::ComponentSender<Self>) -> Self {
        let (ps, pr) = channel();
        thread::spawn(move || {
            let mut state = PlayerState::new();
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            let sink = Sink::try_new(&stream_handle).unwrap();
            sink.set_volume(state.volume);
            let mut t = Instant::now();
            loop {
                thread::sleep(Duration::from_millis(50));
                sender
                    .output(MainMessage::StateUpdated(state.clone()))
                    .unwrap();
                // Add the next song to the queue if the queue is empty
                if sink.empty() && !state.queue.is_empty() {
                    let song = state.queue.pop_front();
                    if let Some(song) = song {
                        let source = song.create_source();
                        match source {
                            Ok(source) => {
                                state.total_duration = mp3_duration::from_path(&song.path).ok();
                                state.now_playing = Some(song);
                                sink.append(source);
                                t = Instant::now();
                            }
                            Err(e) => println!("Error reached when appending: {:#?}", e),
                        }
                    }
                } else if sink.empty() && state.queue.is_empty() {
                    state.now_playing = None;
                    state.current_duration = None;
                    state.total_duration = None;
                }
                if state.now_playing.is_some() && !sink.is_paused() {
                    state.current_duration = Some(t.elapsed().mul_f32(sink.speed()));
                }

                // Handle a message if one is recieved
                let message_or_error = pr.try_recv();
                if let Ok(message) = message_or_error {
                    match message {
                        PlayerMessage::Stop => {
                            state.queue.clear();
                            sink.stop();
                        }
                        PlayerMessage::Pause => sink.pause(),
                        PlayerMessage::Play => {
                            sink.play();
                            t = Instant::now()
                                .checked_sub(
                                    state.current_duration.unwrap_or(Duration::from_secs(0)),
                                )
                                .unwrap();
                        }
                        PlayerMessage::Volume(v) => sink.set_volume(v),
                        PlayerMessage::Skip(list) => {
                            let mut sorted = list.clone();
                            sorted.sort_by(|a, b| b.cmp(a));
                            for index in sorted.as_ref() {
                                match index {
                                    0 => sink.stop(),
                                    _ => {
                                        state.queue.remove(*index - 1);
                                    }
                                }
                            }
                        }
                        PlayerMessage::Add(s) => {
                            state.queue.push_back(s);
                        }
                        PlayerMessage::Clear => state.queue.clear(),
                        PlayerMessage::Speed(s) => {
                            t = Instant::now()
                                .checked_sub(
                                    state
                                        .current_duration
                                        .unwrap_or(Duration::new(0, 0).mul_f32(s)),
                                )
                                .unwrap();
                            sink.set_speed(s);
                        }
                        PlayerMessage::ReOrder(origin, mut dest) => {
                            let elem = state.queue.remove(origin);
                            if let Some(song) = elem {
                                if dest >= origin {
                                    dest -= 1;
                                }
                                state.queue.insert(dest.min(state.queue.len()), song)
                            }
                        }
                        PlayerMessage::Seek(n) => {
                            sink.stop();
                            if let Some(song) = &state.now_playing {
                                match song.create_source() {
                                    Ok(s) => {
                                        let dur = Duration::from_secs(n);
                                        sink.append(s.skip_duration(dur));
                                        t = Instant::now().checked_sub(dur).unwrap();
                                    }
                                    Err(e) => println!("Failed seek because {:?}", e),
                                }
                            }
                        }
                    }
                }
            }
        });
        Self { sender: ps }
    }

    fn update(&mut self, message: Self::Input, _sender: relm4::ComponentSender<Self>) {
        self.sender.send(message).unwrap();
    }
}
