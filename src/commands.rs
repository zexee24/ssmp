pub(crate) enum PlayerMessage {
    Stop,
    Play,
    Pause,
    Skip(Box<[usize]>),
    Volume(f32),
    Add(String),
    Clear,
    Speed(f32),
    ReOrder(usize, usize),
}
