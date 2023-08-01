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
pub mod ui;
pub mod youtube;

use gtk::prelude::*;
use player::Player;
use relm4::component::{
    AsyncComponent, AsyncComponentController, AsyncComponentParts, AsyncController,
};
use relm4::factory::{FactoryVecDeque, FactoryVecDequeGuard};
use relm4::gtk::EntryIconPosition;
use relm4::{prelude::*, AsyncComponentSender};
use song::Song;
use ui::youtube_browser::{YoutubeBrowser, YtMessage};

use std::convert::identity;
use std::rc::Rc;
use std::*;

use crate::files::list_songs;
use crate::player_state::PlayerState;
use crate::ui::song_selecter::SongFile;

use self::commands::PlayerMessage;

const APP_ID: &str = "jere.ssmp";

fn main() {
    let app = RelmApp::new(APP_ID);
    app.run_async::<AppModel>(PlayerState::new());
}

struct AppModel {
    status: PlayerState,
    song_files_factory: FactoryVecDeque<SongFile>,
    song_list: Vec<Song>,
    youtube_searcher: AsyncController<YoutubeBrowser>,
    current_search: String,
}

#[derive(Debug)]
pub enum MainMessage {
    StateUpdated(PlayerState),
    SearchChanged(String),
    FilesChanged,
}

#[relm4::component(async)]
impl AsyncComponent for AppModel {
    type Init = PlayerState;
    type Input = MainMessage;
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
                            player_handler.emit(PlayerMessage::skip_first());
                        },
                        gtk::Image{
                            set_from_icon_name: Some("media-skip-forward")
                        }
                    },
                },
                gtk::Entry{
                    set_icon_from_icon_name: (EntryIconPosition::Secondary, Some("system-search")),
                    connect_changed[sender] => move |entry| {
                        let buffer = entry.buffer();
                        sender.input(MainMessage::SearchChanged(buffer.text().into()))
                    }
                },
                gtk::Box{
                    set_orientation: gtk::Orientation::Horizontal,
                    gtk::ScrolledWindow{
                        set_propagate_natural_height: true,
                        set_propagate_natural_width: true,
                        #[local_ref]
                        song_box -> gtk::Box{
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 5,
                        },
                    },
                    append = model.youtube_searcher.widget(),
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

        let mut song_files_factory =
            FactoryVecDeque::<SongFile>::new(gtk::Box::default(), player_handler.sender());
        let mut g = song_files_factory.guard();
        let song_list = list_songs();
        song_list.iter().for_each(|x| {
            g.push_back(x.clone());
        });
        g.drop();
        let youtube_searcher = YoutubeBrowser::builder()
            .launch(())
            .forward(sender.input_sender(), identity);
        let model = AppModel {
            status,
            song_files_factory,
            song_list,
            youtube_searcher,
            current_search: "".to_string(),
        };
        let song_box = model.song_files_factory.widget();
        let widgets = view_output!();
        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            MainMessage::FilesChanged => {
                // TODO: Check for possible file changes by system periotically

                self.song_list = list_songs();
                let mut g = self.song_files_factory.guard();
                g.clear();
                insert_into_factory(
                    self.song_list
                        .clone()
                        .into_iter()
                        .filter(|x| x.matches_name(&self.current_search)),
                    &mut g,
                );
            }
            MainMessage::StateUpdated(s) => self.status = s,
            MainMessage::SearchChanged(s) => {
                self.current_search = s.clone();
                let mut g = self.song_files_factory.guard();
                g.clear();
                insert_into_factory(
                    self.song_list
                        .clone()
                        .into_iter()
                        .filter(|x| x.matches_name(&s)),
                    &mut g,
                );
                self.youtube_searcher
                    .sender()
                    .emit(YtMessage::QueryChanges(s))
            }
        }
    }
}

pub fn insert_into_factory<I, T, F>(iter: I, g: &mut FactoryVecDequeGuard<F>)
where
    I: Iterator<Item = T>,
    F: FactoryComponent<Init = T, Index = DynamicIndex>,
{
    iter.for_each(|x| {
        g.push_back(x);
    })
}
