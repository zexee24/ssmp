use std::ops::Div;

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
    /// Seeks n seconds into the song,
    Seek(u64),
}

impl PlayerMessage {
    const VOLUME_MAX :f32= 3.0;
    pub fn skip_first() -> Self {
        Self::Skip(Box::new([0]))
    }

    /// Takes input n that is a `f64` value and returns a more intutive version of volume, with the
    /// max being `VOLUME_MAX`. If the given input is over 1.0, it is treated as 1.0
    pub fn exp_volume(n: f64) -> Self{
        let x = n.max(0.0).min(1.0);
        Self::Volume(x.powi(4) as f32 * Self::VOLUME_MAX)
    }

    pub fn reverse_exp_volume(n: f32) -> f64 {
        n.div(Self::VOLUME_MAX).max(0.0).powf(0.25).into()
    }
}

#[test]
fn test_exp_vol() {
    let v = 0.48;
    let diff = match PlayerMessage::exp_volume(v){
        PlayerMessage::Volume(f) => PlayerMessage::reverse_exp_volume(f) - v,
        _ => unreachable!(),

    };
    assert!(diff <= 10e9)
}
