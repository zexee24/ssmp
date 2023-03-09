use crate::song::Song;

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
}
