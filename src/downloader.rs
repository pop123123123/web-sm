use crate::error::*;
use crate::youtube_dl::{Arg, ResultType, YoutubeDL};
use actix::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::vec::Vec;

#[derive(Deserialize)]
pub struct GetVideoPath {
    #[serde(skip)]
    pub yt_id: String,
}
impl actix::Message for GetVideoPath {
    type Result = Result<(), DownloadVideoResult>;
}

#[derive(Deserialize)]
pub struct DownloadVideos {
    #[serde(skip)]
    pub yt_ids: Vec<String>,
}
impl actix::Message for DownloadVideos {
    type Result = Result<(), ServerError>;
}

pub enum DownloadVideoResult {
    NeverDownloaded,
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

async fn download_videos(yt_ids: Vec<String>) -> Result<Vec<String>, ()> {
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

    let max_tries: u8 = 5;
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
                return Ok(yt_ids);
            }
            ResultType::IOERROR | ResultType::FAILURE => {
                println!("Couldn't start download: {}", download.output())
            }
        };
    }
    Err(())
}

impl Actor for DownloaderActor {
    type Context = Context<Self>;
}

impl Handler<DownloadVideos> for DownloaderActor {
    type Result = ResponseActFuture<Self, Result<(), ServerError>>;

    fn handle(&mut self, msg: DownloadVideos, ctx: &mut Context<Self>) -> Self::Result {
        let wrap_fut = if msg
            .yt_ids
            .iter()
            .any(|url| !self.download_states.contains_key(url))
        {
            msg.yt_ids.iter().for_each(|yt_id| {
                self.download_states.insert(yt_id.clone(), false);
            });

            let wrap_download = actix::fut::wrap_future(download_videos(msg.yt_ids));
            let mapped_download = wrap_download.map(
                |result: Result<Vec<String>, ()>, actor: &mut DownloaderActor, _ctx| match result {
                    Ok(yt_ids) => {
                        yt_ids.into_iter().for_each(|yt_id| {
                            actor.download_states.insert(yt_id, true);
                        });
                        Ok(())
                    }
                    Err(_e) => Err(ServerError::CommunicationError),
                },
            );
            fut::Either::Left(mapped_download)
        } else {
            fut::Either::Right(actix::fut::wrap_future(async {
                Ok(())
                // if msg.yt_ids.into_iter().all(|url| get_video(&url).is_none()) {
                //     Ok(())
                // } else {
                //     Ok(())
                // }
            }))
        };
        Box::pin(wrap_fut)
    }
}

impl Handler<GetVideoPath> for DownloaderActor {
    type Result = Result<(), DownloadVideoResult>;

    fn handle(&mut self, msg: GetVideoPath, ctx: &mut Context<Self>) -> Self::Result {
        let yt_id = msg.yt_id;
        match self.download_states.get(&yt_id) {
            Some(finished) => {
                if *finished {
                    Ok(())
                } else {
                    Err(DownloadVideoResult::DownloadPending)
                }
            }
            None => Err(DownloadVideoResult::NeverDownloaded),
        }
    }
}
