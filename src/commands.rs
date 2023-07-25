use crate::song::Song;

#[derive(Debug)]
pub enum PlayerMessage {
    Stop,
    Play,
    Pause,
    Skip(Box<[usize]>),
    Volume(f32),
    Add(Song),
    Clear,
    Speed(f32),
    ReOrder(usize, usize),
    Seek(u64),
}

impl PlayerMessage {
    pub fn skip_first() -> Self {
        Self::Skip(Box::new([0]))
    }
}
