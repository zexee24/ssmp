use relm4::{
    component::{AsyncComponent, AsyncComponentParts},
    factory::FactoryVecDeque,
    loading_widgets::LoadingWidgets,
    view, AsyncComponentSender,
};

use gtk::prelude::*;
use relm4::gtk;
use reqwest::Client;

use crate::{
    downloader::download_dlp,
    insert_into_factory,
    youtube::{scrape_youtube, video::Video},
    MainMessage,
};

#[derive(Debug)]
pub struct YoutubeBrowser {
    youtube_factory: FactoryVecDeque<Video>,
    client: Client,
}

#[derive(Debug)]
pub enum YtMessage {
    Download(String),
    QueryChanges(String),
}

#[relm4::component(async, pub)]
impl AsyncComponent for YoutubeBrowser {
    type Init = ();
    type Input = YtMessage;
    type Output = MainMessage;
    type CommandOutput = ();
    view! {
        gtk::Box{
            #[local_ref]
            yt_box -> gtk::Box{
                set_orientation: gtk::Orientation::Vertical,
            }
        }
    }

    async fn init(
        _: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let youtube_factory =
            FactoryVecDeque::<Video>::new(gtk::Box::default(), sender.input_sender());
        let model = YoutubeBrowser {
            youtube_factory,
            client: Client::new(),
        };
        let yt_box = model.youtube_factory.widget();
        let widgets = view_output!();
        AsyncComponentParts { model, widgets }
    }
    fn init_loading_widgets(root: &mut Self::Root) -> Option<LoadingWidgets> {
        view! {
            #[name(spinner)]
            gtk::Spinner {
                start: (),
                set_halign: gtk::Align::Center,
            }
        }
        Some(LoadingWidgets::new(root, spinner))
    }
    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            YtMessage::Download(id) => {
                match download_dlp(format!("https://www.youtube.com/watch?v={}", id)).await {
                    Ok(_) => {
                    // PERF: This should be done without scanning the whole file system
                        sender.output(MainMessage::FilesChanged).unwrap();
                    }
                    Err(e) => println!("Error: {}", e),
                }
            }
            YtMessage::QueryChanges(s) => {
                // FIX: This should either run in the background or only be run manually
                let results = scrape_youtube(&s, &self.client).await;
                match results {
                    Ok(r) => {
                        let mut g = self.youtube_factory.guard();
                        g.clear();
                        insert_into_factory(r.into_iter(), &mut g)
                    }
                    // TODO: Make errors visible in the program
                    Err(e) => println!("{}", e),
                }
            }
        }
    }
}
