use std::process::Command;

use crate::data::{AmbiguityError, AnalysisResult, Project};

fn get_command() -> Command {
    cfg_if::cfg_if! {
      if #[cfg(target_os = "windows")] {
      Command::new(".\\sm-interface\\launch")
    } else {
      Command::new("./sm-interface/launch")
    }
    }
}

pub fn analyze(project: &Project, sentence: &str) -> Result<AnalysisResult, AmbiguityError> {
    let mut command = get_command();
    let output = command
        .args(&[sentence, &project.seed])
        .args(&project.video_urls)
        .output()
        .expect("Couldn't launch sm-interface.");

    let out_data = output.stdout;
    let res: serde_json::Result<AnalysisResult> = serde_json::from_slice(&out_data);
    let res: Result<AnalysisResult, AmbiguityError> = res.map_err(|_| -> AmbiguityError {
        serde_json::from_slice(&out_data).expect("Unrecognized output from sm-interface.")
    });
    res
}
