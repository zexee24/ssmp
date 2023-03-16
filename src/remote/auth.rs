use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Permission {
    VolumeControl((f32, f32)),
    Seek,
    Add,
    Download,
    PlayPause,
    Info,
    All,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Key {
    pub key: String,
    pub permissions: Vec<Permission>,
}

impl Default for Key {
    fn default() -> Self {
        Key {
            key: "".to_owned(),
            permissions: vec![Permission::Add, Permission::Download, Permission::Info],
        }
    }
}

impl Key {
    pub fn convert_all(&mut self) {
        if self.permissions.contains(&Permission::All) {
            self.permissions = vec![
                Permission::VolumeControl((0.0, 10.0)),
                Permission::Seek,
                Permission::All,
                Permission::Download,
                Permission::PlayPause,
                Permission::Info,
            ]
        }
    }
}
