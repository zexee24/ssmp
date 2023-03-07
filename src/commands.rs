pub(crate) enum PlayerMessage {
    Stop,
    Play,
    Pause,
    Skip(usize),
    Volume(f32),
    Add(String),
    Clear,
    Speed(f32),
    SkipList(Box<[usize]>),
    ReOrder(usize, usize),
}
