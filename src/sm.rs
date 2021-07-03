use chashmap::CHashMap;
use once_cell::sync::Lazy;
use std::sync::{RwLock, Arc};
use tokio::process::Command;

use crate::data::{AmbiguityError, AnalysisResult, AnalysisId, Project};

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

pub async fn analyze(project: &Project, sentence: &str) -> Result<Arc<AnalysisResult>, AmbiguityError> {
    let hash_key = AnalysisId::from_project_sentence(project, sentence);
    match ANALYSIS_CACHE.read().unwrap().get(&hash_key) {
        Some(result) => Ok((*result).clone()),
        None => {
            let mut command = get_command();
            let output = command
                .args(&[sentence, &project.seed])
                .args(&project.video_urls)
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
            let res: Result<Arc<AnalysisResult>, AmbiguityError> = res.map_err(|_| -> AmbiguityError {
                match serde_json::from_slice(&out_data) {
                    Ok(res) => res,
                    Err(_) => panic!("{}", String::from_utf8(err_data).unwrap()),
                }
            });
            res
        },
    }
}
