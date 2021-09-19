use crate::data::{AmbiguityError, AnalysisId, AnalysisResult, Project, Video};
use chashmap::CHashMap;
use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock};
use tokio::process::Command;

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
    ANALYSIS_CACHE.read().unwrap().insert(key, val); // panics if panic already happened
}

pub async fn analyze(
    project: &Project,
    sentence: &str,
) -> Result<Arc<AnalysisResult>, AmbiguityError> {
    let hash_key = AnalysisId::from_project_sentence(project, sentence);
    match ANALYSIS_CACHE.read().unwrap().get(&hash_key) {
        // panics if panic already happened
        Some(result) => Ok((*result).clone()),
        None => {
            let urls = project
                .video_ids
                .iter()
                .map(|yt_id| yt_id.id.clone())
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
            let first = out_data[..(out_data.len() - 1)]
                .iter()
                .rposition(|x| *x == 0xa)
                .unwrap_or(0);
            let res: serde_json::Result<AnalysisResult> =
                serde_json::from_slice(&out_data[first..]);
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
                            "STDOUT\n{}\n\nSTDERR\n{}",
                            String::from_utf8(out_data).unwrap(), // panics in panic
                            String::from_utf8(err_data).unwrap(), // panics in panic
                        ),
                    }
                });
            res
        }
    }
}
