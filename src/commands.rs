pub(crate) enum PlayerMessage {
    Stop,
    Play,
    Pause,
    Skip(u64),
    Volume(f32),
    Add(String),
    Clear,
    Speed(f32),
}