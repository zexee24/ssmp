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
    pub elapsed_duration: Option<Duration>,
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
            elapsed_duration: None,
        }
    }

    pub fn show_total_duration(&self) -> Option<String>{
        Some(Self::display_duration(self.total_duration?))
    }

    pub fn show_elapsed_duration(&self) -> Option<String>{
        Some(Self::display_duration(self.elapsed_duration?))
    }

    fn display_duration(d: Duration) -> String{
        let secs = d.as_secs();
        let display_secs = match secs % 60 < 10 {
            true => format!("0{}", secs % 60),
            false => format!("{}",secs % 60),
        };
        format!("{}:{}", secs/60, display_secs)
    }
}
