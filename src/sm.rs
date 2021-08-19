use chashmap::CHashMap;
use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock};
use tokio::process::Command;
use ytd_rs::{YoutubeDL, ResultType, Arg};
use std::path::PathBuf;
use std::error::Error;
use crate::data::{AmbiguityError, AnalysisId, AnalysisResult, Project, Video};

fn get_command() -> Command {
    cfg_if::cfg_if! {
      if #[cfg(target_os = "windows")] {
      Command::new(".\\sm-interface\\launch")
    } else {
      Command::new("./sm-interface/launch")
    }
    }
}

type AnalysisCache = CHashMap<AnalysisId, Arc<AnalysisResult>>;
static ANALYSIS_CACHE: Lazy<RwLock<AnalysisCache>> =
    Lazy::new(|| RwLock::new(AnalysisCache::new()));

fn add_in_cache(key: AnalysisId, val: Arc<AnalysisResult>) {
    ANALYSIS_CACHE.read().unwrap().insert(key, val);
}

pub async fn download_videos(
    videos: Vec<Video>,
) -> () {
    let args = vec![Arg::new_with_arg("--output", "%(id)s")];

    // Align all the urls on a same string
    let mut inline_urls = videos.iter().fold(String::from(""), |mut full_str, v| {
        full_str.push_str(&v.url);
        full_str.push_str(" ");
        full_str
    });
    inline_urls.pop(); // Remove last superfluous " "

    let path = PathBuf::from("./.videos");
    let ytd = YoutubeDL::new(&path, args, &inline_urls.clone()).unwrap();

    // start download
    let download = ytd.download();

    // check what the result is and print out the path to the download or the error
    match download.result_type() {
        ResultType::SUCCESS => println!("Videos downloaded: {}", download.output_dir().to_string_lossy()),
        ResultType::IOERROR | ResultType::FAILURE =>
                println!("Couldn't start download: {}", download.output()),
    };
}

pub async fn analyze(
    project: &Project,
    sentence: &str,
) -> Result<Arc<AnalysisResult>, AmbiguityError> {
    let hash_key = AnalysisId::from_project_sentence(project, sentence);
    match ANALYSIS_CACHE.read().unwrap().get(&hash_key) {
        Some(result) => Ok((*result).clone()),
        None => {
            let urls = project
                .video_urls
                .iter()
                .map(|v| v.url.clone())
                .collect::<Vec<_>>();
            let mut command = get_command();
            let output = command
                .args(&[sentence, &project.seed])
                .args(&urls)
                .output()
                .await
                .expect("Couldn't launch sm-interface.");

            let err_data = output.stderr;
            let out_data = output.stdout;
            let res: serde_json::Result<AnalysisResult> = serde_json::from_slice(&out_data);
            let res = res.map(|res| {
                let boxed = Arc::new(res);
                add_in_cache(hash_key, boxed.clone());
                boxed
            });
            let res: Result<Arc<AnalysisResult>, AmbiguityError> =
                res.map_err(|_| -> AmbiguityError {
                    match serde_json::from_slice(&out_data) {
                        Ok(res) => res,
                        Err(_) => panic!(
                            "{}{}",
                            String::from_utf8(out_data).unwrap(),
                            String::from_utf8(err_data).unwrap()
                        ),
                    }
                });
            res
        }
    }
}
