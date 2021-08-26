use crate::error::*;
use crate::youtube_dl::{Arg, ResultType, YoutubeDL};
use actix::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::vec::Vec;

#[derive(Deserialize)]
pub struct GetVideoPath {
    #[serde(skip)]
    pub yt_id: String,
}
impl actix::Message for GetVideoPath {
    type Result = Option<String>;
}

#[derive(Deserialize)]
pub struct DownloadVideos {
    #[serde(skip)]
    pub yt_ids: Vec<String>,
}
impl actix::Message for DownloadVideos {
    type Result = Result<DownloadVideoResult, ServerError>;
}

pub enum DownloadVideoResult {
    DonwloadStarted,
    DownloadPending,
    AlreadyDownload,
}

pub struct DownloaderActor {
    download_states: HashMap<String, bool>,
}

impl DownloaderActor {
    pub fn new() -> DownloaderActor {
        DownloaderActor {
            download_states: HashMap::new(),
        }
    }
}

fn get_video(yt_id: &str) -> Option<&str> {
    Some("coucou")
}

impl DownloaderActor {
    pub async fn download_videos(&mut self, yt_ids: Vec<String>) -> Result<(), ()> {
        println!("PING");
        yt_ids.iter().for_each(|yt_id| {
            self.download_states.insert(yt_id.clone(), false);
        });

        let args = vec![Arg::new_with_arg("--output", "%(id)s")];

        // Align all the urls on a same string
        let mut inline_urls = yt_ids.iter().fold(String::from(""), |mut full_str, yt_id| {
            full_str.push_str(yt_id);
            full_str.push_str(" ");
            full_str
        });

        inline_urls.pop(); // Remove last superfluous " "

        let path = PathBuf::from("./.videos");
        let ytd = YoutubeDL::new(&path, args, &inline_urls.clone()).unwrap();

        let max_tries = 5;
        for i_try in 0..max_tries {
            println!("Downloading videos {}. Try {}", inline_urls, i_try);

            // start download
            let download = ytd.download().await;

            // check what the result is and print out the path to the download or the error
            match download.result_type() {
                ResultType::SUCCESS => {
                    println!(
                        "Videos downloaded: {}",
                        download.output_dir().to_string_lossy()
                    );
                    yt_ids.into_iter().for_each(|yt_id| {
                        self.download_states.insert(yt_id, true);
                    });
                    return Ok(());
                }
                ResultType::IOERROR | ResultType::FAILURE => {
                    println!("Couldn't start download: {}", download.output())
                }
            };
        }
        Err(())
    }
}

impl Actor for DownloaderActor {
    type Context = Context<Self>;
}

impl Handler<DownloadVideos> for DownloaderActor {
    type Result = Result<DownloadVideoResult, ServerError>;

    fn handle(&mut self, msg: DownloadVideos, ctx: &mut Context<Self>) -> Self::Result {
        if msg
            .yt_ids
            .into_iter()
            .any(|url| !self.download_states.contains_key(&url))
        {
            let fut = self.download_videos(msg.yt_ids);
            actix::Arbiter::spawn(async {
                fut.await;
            });

            Ok(DownloadVideoResult::DonwloadStarted)
        } else {
            if msg.yt_ids.into_iter().any(|url| get_video(&url).is_none()) {
                return Ok(DownloadVideoResult::DownloadPending);
            } else {
                return Ok(DownloadVideoResult::AlreadyDownload);
            }
        }
    }
}

impl Handler<GetVideoPath> for DownloaderActor {
    type Result = Option<String>;

    fn handle(&mut self, msg: GetVideoPath, ctx: &mut Context<Self>) -> Self::Result {
        get_video(&msg.yt_id)
    }
}
