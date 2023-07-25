use std::{collections::VecDeque, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{conf::Configuration, song::Song};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerState {
    pub now_playing: Option<Song>,
    pub queue: VecDeque<Song>,
    pub volume: f32,
    pub speed: f32,
    pub paused: bool,
    pub total_duration: Option<Duration>,
    pub current_duration: Option<Duration>,
}

impl PlayerState {
    pub fn new() -> Self {
        let vol = Configuration::get_conf().default_volume;
        Self {
            now_playing: None,
            queue: VecDeque::new(),
            volume: vol,
            speed: 1.0,
            paused: false,
            total_duration: None,
            current_duration: None,
        }
    }
}
