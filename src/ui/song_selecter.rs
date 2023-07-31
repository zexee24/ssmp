use relm4::{
    gtk::{
        self,
        traits::{BoxExt, ButtonExt},
    },
    prelude::FactoryComponent,
    FactorySender,
};

use crate::{commands::PlayerMessage, song::Song};

#[derive(Debug)]
pub(crate) struct SongFile {
    song: Song,
}

/// This thing has to exist because `relm4::factory` cannot handle cloning self.
#[derive(Debug)]
pub enum SelectorMessage {
    Queue,
    QueueFront,
    Play,
}

#[relm4::factory(pub)]
impl FactoryComponent for SongFile {
    type Init = Song;
    type Input = SelectorMessage;
    type Output = PlayerMessage;
    type CommandOutput = ();
    type Widgets = SongFileWidgets;
    type ParentInput = PlayerMessage;
    type ParentWidget = gtk::Box;

    view! {
        root = gtk::Box{
            set_spacing: 3,
            gtk::Label{
                #[watch]
                set_label: &self.song.name,
            },
            gtk::Button{
                gtk::Image{
                    set_from_icon_name: Some("media-playback-start"),
                },
                connect_clicked => SelectorMessage::Play
            },
            gtk::Button{
                gtk::Image{
                    set_from_icon_name: Some("view-continuous"),
                },
                connect_clicked => SelectorMessage::Queue
            }
        }
    }

    fn init_model(
        init: Self::Init,
        _index: &Self::Index,
        _sender: relm4::FactorySender<Self>,
    ) -> Self {
        Self { song: init }
    }

    fn forward_to_parent(output: Self::Output) -> Option<Self::Output> {
        Some(output)
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            SelectorMessage::Queue => sender.output(PlayerMessage::Add(self.song.clone())),
            SelectorMessage::Play => {
                sender.output(PlayerMessage::Stop);
                sender.output(PlayerMessage::Add(self.song.clone()));
            }
            _ => {}
        }
    }
}
