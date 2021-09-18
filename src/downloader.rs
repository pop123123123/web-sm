use crate::data::Video;
use crate::error::*;
use crate::renderer::render_main_video;
use crate::youtube_dl::{Arg, ResultType, YoutubeDL};
use actix::*;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{self, DirEntry};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::vec::Vec;

#[derive(Deserialize)]
pub struct GetVideoPath {
    #[serde(skip)]
    pub yt_ids: Vec<String>,
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
    AlreadyDownloaded,
}

pub struct DownloaderActor {
    download_states: HashMap<String, bool>,
    videos: HashMap<String, Video>,
}

impl DownloaderActor {
    pub fn new() -> DownloaderActor {
        DownloaderActor {
            download_states: HashMap::new(),
            videos: HashMap::new(),
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
    let ytd = YoutubeDL::new_multiple_links(&path, args, yt_ids.clone()).unwrap();

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
                        yt_ids.iter().for_each(|yt_id| {
                            actor.download_states.insert(yt_id.to_owned(), true);

                            let vid: Video = Video { url: yt_id.clone() };
                            let path = get_video_path(yt_id).unwrap();
                            crate::renderer::render_main_video(
                                path.as_path(),
                                vid.get_path_full_resolution().as_path(),
                                false,
                            )
                            .unwrap();
                            crate::renderer::render_main_video(
                                path.as_path(),
                                vid.get_path_small().as_path(),
                                true,
                            )
                            .unwrap();

                            actor.videos.insert(yt_id.clone(), vid);
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

fn get_video_path(url: &str) -> Option<PathBuf> {
    let pattern = format!(r"^.*{}..*$", url);
    let re = Regex::new(&pattern).unwrap();

    let found_path = fs::read_dir(".videos").unwrap().find(|entry| {
        let path = entry.as_ref().unwrap().path();
        let path_str = path.into_os_string().into_string().unwrap();
        re.is_match(&path_str)
    });
    match found_path {
        Some(entry) => Some(entry.as_ref().unwrap().path().canonicalize().unwrap()),
        None => None,
    }
}

impl Handler<GetVideoPath> for DownloaderActor {
    type Result = Result<(), DownloadVideoResult>;

    fn handle(&mut self, msg: GetVideoPath, ctx: &mut Context<Self>) -> Self::Result {
        let yt_ids = msg.yt_ids;

        let status = yt_ids
            .iter()
            .map(|yt_id| match self.download_states.get(yt_id) {
                Some(finished) => {
                    if *finished {
                        DownloadVideoResult::AlreadyDownloaded
                    } else {
                        DownloadVideoResult::DownloadPending
                    }
                }
                None => DownloadVideoResult::NeverDownloaded,
            })
            .find(|status| !matches!(status, DownloadVideoResult::AlreadyDownloaded));

        match status {
            Some(s) => Err(s),
            None => Ok(()),
        }
    }
}
