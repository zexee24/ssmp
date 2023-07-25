pub mod commands;
pub mod conf;
pub mod console;
pub mod downloader;
pub mod files;
pub mod format;
mod player;
pub mod player_state;
pub mod remote;
pub mod song;

use gtk::prelude::*;
use player::Player;
use relm4::component::{AsyncComponent, AsyncComponentParts};
use relm4::{prelude::*, AsyncComponentSender};

use std::convert::identity;
use std::rc::Rc;
use std::*;

use crate::player_state::PlayerState;
use crate::song::Song;

use self::commands::PlayerMessage;

const APP_ID: &str = "jere.ssmp";

fn main() {
    let app = RelmApp::new(APP_ID);
    app.run_async::<AppModel>(PlayerState::new());
}

struct AppModel {
    status: PlayerState,
}

#[relm4::component(async)]
impl AsyncComponent for AppModel {
    type Init = PlayerState;
    type Input = PlayerState;
    type Output = PlayerMessage;
    type CommandOutput = ();
    view! {
        gtk::Window {
            set_title: Some("Socially Shared Music Player"),
            set_default_size: (300, 100),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 5,
                set_margin_all: 5,

                gtk::Label {
                    #[watch]
                    set_label: &format!("{}", model.status.now_playing.clone().map(|x| x.name).unwrap_or("".to_string())),
                    set_margin_all: 5,
                },
                gtk::Box{
                    set_orientation: gtk::Orientation::Horizontal,

                    gtk::Image{

                    }
                },
                gtk::Box{
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 10,
                    set_halign: gtk::Align::Center,
                    gtk::Button{
                        connect_clicked[player_handler] => move |_| {
                            player_handler.emit(PlayerMessage::Seek(0));
                        },
                        gtk::Image{
                            set_from_icon_name: Some("media-skip-backward")
                        }
                    },
                    gtk::Box{
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 2,
                        #[name = "group"]
                        gtk::ToggleButton{
                            gtk::Image{
                                set_from_icon_name: Some("media-playback-start")
                            },
                            set_active: !model.status.paused,
                            connect_clicked[player_handler] => move |_| {
                                player_handler.emit(PlayerMessage::Play);
                            }
                        },
                        gtk::ToggleButton{
                            gtk::Image{
                                set_from_icon_name: Some("media-playback-pause")
                            },
                            set_active: model.status.paused,
                            set_group: Some(&group),
                            connect_clicked[player_handler] => move |_| {
                                player_handler.emit(PlayerMessage::Pause);
                            }
                        }
                    },

                    gtk::Button{
                        connect_clicked[player_handler] => move |_| {
                            player_handler.emit(PlayerMessage::Add(Song::from_file("songs/haloo helsinki - ei eerika pääse taivaaseen.mp3".into()).unwrap()));
                            //player_handler.emit(PlayerMessage::skip_first());
                        },
                        gtk::Image{
                            set_from_icon_name: Some("media-skip-forward")
                        }
                    },
                }
            }
        }
    }

    async fn init(
        status: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let player_handler = Rc::new(
            Player::builder()
                .detach_worker(())
                .forward(sender.input_sender(), identity),
        );
        let model = AppModel { status };
        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        self.status = msg;
    }
}
