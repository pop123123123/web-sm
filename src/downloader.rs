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
    type Result = Result<(), ServerError>;
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

async fn download_videos(yt_ids: Vec<YoutubeId>) -> Result<Vec<YoutubeId>, Vec<YoutubeId>> {
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
                return Ok(yt_ids);
            }
            ResultType::IOERROR | ResultType::FAILURE => {
                println!("Couldn't start download: {}", download.output())
            }
        };
    }
    Err(yt_ids)
}

impl Actor for DownloaderActor {
    type Context = Context<Self>;
}

impl Handler<DownloadVideos> for DownloaderActor {
    type Result = ResponseActFuture<Self, Result<(), ServerError>>;

    fn handle(&mut self, msg: DownloadVideos, _ctx: &mut Context<Self>) -> Self::Result {
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
                |result: Result<Vec<YoutubeId>, Vec<YoutubeId>>,
                 actor: &mut DownloaderActor,
                 _ctx| match result {
                    Ok(yt_ids) => {
                        yt_ids.iter().for_each(|yt_id| {
                            actor.download_states.insert(yt_id.clone(), true);

                            let path = get_video_path(yt_id).unwrap();
                            crate::renderer::render_main_video(
                                path.as_path(),
                                crate::data::get_video_path(&yt_id.id, false).as_path(),
                                false,
                            )
                            .unwrap();
                            crate::renderer::render_main_video(
                                path.as_path(),
                                crate::data::get_video_path(&yt_id.id, true).as_path(),
                                true,
                            )
                            .unwrap();

                            let vid = Arc::new(Video::from(yt_id.clone()).unwrap());
                            actor.videos.insert(yt_id.clone(), vid);
                        });

                        Ok(())
                    }
                    Err(yt_ids) => {
                        yt_ids.iter().for_each(|yt_id| {
                            actor.download_states.remove(yt_id);
                        });
                        todo!(
                            "Check which videos are correctly downloaded, and which videos are not"
                        );
                        Err(ServerError::CommunicationError)
                    }
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

fn get_video_path(id: &YoutubeId) -> Option<PathBuf> {
    let pattern = format!(r"^.*{}..*$", id.id);
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
            None => Ok(videos.into_iter().map(|v| (*v.unwrap()).clone()).collect()),
        }
    }
}
