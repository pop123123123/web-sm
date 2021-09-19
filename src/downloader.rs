use crate::data::{Video, YoutubeId};
use crate::error::*;
use crate::youtube_dl::{Arg, ResultType, YoutubeDL};
use actix::*;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::vec::Vec;

#[derive(Deserialize)]
pub struct DownloadVideos {
    #[serde(skip)]
    pub yt_ids: Vec<YoutubeId>,
}
impl actix::Message for DownloadVideos {
    type Result = Result<(), DownloaderError>;
}

#[derive(Deserialize)]
pub struct GetVideos {
    #[serde(skip)]
    pub yt_ids: Vec<YoutubeId>,
}
impl actix::Message for GetVideos {
    type Result = Result<Vec<Arc<Video>>, DownloadVideoStatus>;
}

#[derive(Debug)]
pub enum DownloadVideoStatus {
    NeverDownloaded,
    DownloadPending,
}

pub struct DownloaderActor {
    download_states: HashMap<YoutubeId, bool>,
    videos: HashMap<YoutubeId, Arc<Video>>,
}

impl DownloaderActor {
    pub fn new() -> DownloaderActor {
        DownloaderActor {
            download_states: HashMap::new(),
            videos: HashMap::new(),
        }
    }
}

async fn download_videos(yt_ids: Vec<YoutubeId>) -> Result<(), ()> {
    let args = vec![Arg::new_with_arg("--output", "%(id)s")];

    // Align all the urls on a same string
    let mut inline_urls = yt_ids.iter().fold(String::from(""), |mut full_str, yt_id| {
        full_str.push_str(&yt_id.id);
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
                return Ok(());
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
    type Result = ResponseActFuture<Self, Result<(), DownloaderError>>;

    fn handle(&mut self, msg: DownloadVideos, _ctx: &mut Context<Self>) -> Self::Result {
        let wrap_fut = if msg
            .yt_ids
            .iter()
            .any(|url| !self.download_states.contains_key(url))
        {
            msg.yt_ids.iter().for_each(|yt_id| {
                self.download_states.insert(yt_id.clone(), false);
            });

            let yt_ids = msg.yt_ids.clone();

            let wrap_download = actix::fut::wrap_future(download_videos(msg.yt_ids));
            let mapped_download = wrap_download.map(
                move |result: Result<(), ()>, actor: &mut DownloaderActor, _ctx| match result {
                    Ok(()) => {
                        yt_ids.iter().try_for_each(|yt_id| {
                            actor.download_states.insert(yt_id.clone(), true);

                            let path = get_video_path(yt_id);
                            // Error while getting video path
                            let path =
                                path.map_err(|_| DownloaderError::VideosFolderNotExistError)?;
                            if path.is_none() {
                                // No video file matching this video was found
                                return Err(DownloaderError::DowloadedVideoNotFoundError);
                            };
                            let path = path.unwrap();

                            crate::renderer::render_main_video(
                                path.as_path(),
                                crate::data::get_video_path(&yt_id.id, false).as_path(),
                                false,
                            )
                            .map_err(|_| DownloaderError::RenderingError)?;
                            crate::renderer::render_main_video(
                                path.as_path(),
                                crate::data::get_video_path(&yt_id.id, true).as_path(),
                                true,
                            )
                            .map_err(|_| DownloaderError::RenderingError)?;

                            let vid = Arc::new(
                                Video::from(yt_id.clone())
                                    .map_err(|_| DownloaderError::BrokenRenderedVideo)?,
                            );
                            actor.videos.insert(yt_id.clone(), vid);

                            Ok(())
                        })
                    }
                    Err(()) => {
                        yt_ids.iter().for_each(|yt_id| {
                            actor.download_states.remove(yt_id);
                        });
                        // TODO: Check which videos are correctly downloaded, and which videos are not
                        Err(DownloaderError::DownloadFailedError)
                    }
                },
            );
            fut::Either::Left(mapped_download)
        } else {
            fut::Either::Right(actix::fut::wrap_future(async { Ok(()) }))
        };
        Box::pin(wrap_fut)
    }
}

fn get_video_path(id: &YoutubeId) -> std::io::Result<Option<PathBuf>> {
    let pattern = format!(r"^.*{}..*$", id.id);
    let re = Regex::new(&pattern).unwrap(); // unwrap fails if the pattern is invalid. Our pattern is static so never invalid

    let found_path = fs::read_dir(".videos")?.find(|entry| {
        let entry = entry.as_ref();
        if entry.is_err() {
            // Any IO error with read file
            return false;
        }
        let path = entry.unwrap().path();
        let path_str = path.into_os_string().into_string();
        if entry.is_err() {
            // Non-unicode character
            return false;
        }
        re.is_match(&path_str.unwrap())
    });
    match found_path {
        // Both unwraps are validated by the find closure
        Some(entry) => Ok(Some(entry.as_ref().unwrap().path().canonicalize().unwrap())),
        None => Ok(None),
    }
}

impl Handler<GetVideos> for DownloaderActor {
    type Result = Result<Vec<Arc<Video>>, DownloadVideoStatus>;

    fn handle(&mut self, msg: GetVideos, _ctx: &mut Context<Self>) -> Self::Result {
        let yt_ids = msg.yt_ids;

        let videos: Vec<Option<&Arc<Video>>> =
            yt_ids.iter().map(|yt_id| self.videos.get(&yt_id)).collect();

        match videos.iter().position(|v| v.is_none()) {
            Some(v_index) => match self.download_states.get(&yt_ids[v_index]) {
                Some(_) => Err(DownloadVideoStatus::DownloadPending),
                None => Err(DownloadVideoStatus::NeverDownloaded),
            },
            // unwrap validated since we know that v.is_ok is true
            None => Ok(videos.into_iter().map(|v| (*v.unwrap()).clone()).collect()),
        }
    }
}
