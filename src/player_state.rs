use std::{collections::VecDeque, time::Duration};

use serde::{Deserialize, Serialize};

use crate::song::Song;

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerState {
    pub now_playing: Option<Song>,
    pub queue: VecDeque<Song>,
    pub volume: f32,
    pub speed: f32,
    pub paused: bool,
    pub total_duration: Option<Duration>,
    pub current_duration: Option<Duration>,
}
