use relm4::{gtk, FactorySender};
use std::time::Duration;

use relm4::gtk::traits::BoxExt;
use relm4::gtk::traits::ButtonExt;
use relm4::prelude::FactoryComponent;

use crate::ui::youtube_browser::YtMessage;

#[derive(Debug, PartialEq, Clone)]
pub struct Video {
    pub title: String,
    pub artist: String,
    pub length: Duration,
    pub id: String,
    pub thumbnail: String,
}

#[derive(Debug)]
pub enum VideoMessage {
    Download,
}

#[relm4::factory(pub)]
impl FactoryComponent for Video {
    type Init = Video;
    type Input = VideoMessage;
    type Output = YtMessage;
    type CommandOutput = ();
    type Widgets = SongFileWidgets;
    type ParentInput = YtMessage;
    type ParentWidget = gtk::Box;

    view! {
        root = gtk::Box{
            set_spacing: 3,
            gtk::Label{
                #[watch]
                set_label: &self.title,
            },
            gtk::Button{
                gtk::Image{
                    set_from_icon_name: Some("folder-download-symbolic")
                },
                connect_clicked => VideoMessage::Download
            }
        }
    }

    fn init_model(
        init: Self::Init,
        _index: &Self::Index,
        _sender: relm4::FactorySender<Self>,
    ) -> Self {
        init
    }

    fn forward_to_parent(output: Self::Output) -> Option<Self::Output> {
        Some(output)
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            VideoMessage::Download => sender.output(YtMessage::Download(self.id.clone())),
        }
    }
}
