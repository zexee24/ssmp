use std::{collections::VecDeque, time::Duration};

use crate::song::Song;

#[derive(Debug)]
pub(crate) struct  PlayerState {
    pub now_playing : Option<Song>,
    pub queue : VecDeque<Song>,
    pub volume : f32,
    pub speed : f32,
    pub paused : bool,
    pub source_duration : Option<Duration>,
}