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
    song::Song,
    youtube::{scrape_youtube, video::Video},
    MainMessage,
};

#[derive(Debug)]
pub struct YoutubeBrowser {
    youtube_factory: FactoryVecDeque<Video>,
    order: u128,
    view_order: u128,
}

#[derive(Debug)]
pub enum YtMessage {
    // PERF: Investigate if there is a performance diffirence when using other sizes
    Download(String),
    QueryChanges(String),
}

#[derive(Debug)]
pub enum CommandMessage {
    QueryUpdated(Vec<Video>, u128),
    QueryFailed(String),
    DownloadSuccesful(Song),
    DownloadFailed(String),
}

#[relm4::component(async, pub)]
impl AsyncComponent for YoutubeBrowser {
    type Init = ();
    type Input = YtMessage;
    type Output = MainMessage;
    type CommandOutput = CommandMessage;
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
        let model = YoutubeBrowser { youtube_factory, order: 0, view_order: 0};
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
            YtMessage::Download(id) => sender.oneshot_command(async move {
                match download_dlp(format!("https://www.youtube.com/watch?v={}", id)).await {
                    Ok(s) => CommandMessage::DownloadSuccesful(s),
                    Err(e) => CommandMessage::DownloadFailed(e),
                }
            }),
            YtMessage::QueryChanges(s) => {
                let order = self.order;
                self.order += 1;
                sender.oneshot_command(async move {
                    // PERF: Use a persistent group of clients, not create new ones for every request
                    match scrape_youtube(&s, &Client::new()).await {
                        Ok(v) => CommandMessage::QueryUpdated(v, order),
                        Err(e) => CommandMessage::QueryFailed(e.to_string()),
                    }
                });
            }
        }
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        _: &Self::Root,
    ) {
        match message {
            CommandMessage::QueryUpdated(v, ord) => {
                if self.view_order < ord{
                    let mut g = self.youtube_factory.guard();
                    g.clear();
                    insert_into_factory(v.into_iter(), &mut g);
                    self.view_order = ord;
                }
            }
            CommandMessage::QueryFailed(s) => println!("Failed to query yt: {}", s),
            CommandMessage::DownloadSuccesful(_) => {
                // PERF: This should be done without scanning the whole file system
                sender.output(MainMessage::FilesChanged).unwrap();
            }
            CommandMessage::DownloadFailed(s) => println!("Failed download: {}", s),
        }
    }
}
