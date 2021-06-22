use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Phonem {
    #[serde(rename = "v")]
    pub video_index: u8,
    #[serde(rename = "s")]
    pub start: f64,
    #[serde(rename = "e")]
    pub end: f64,
}

pub type Combo = Vec<Phonem>;
pub type AnalysisResult = Vec<Combo>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub seed: String,
    pub video_urls: Vec<String>,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AmbiguityError {
    pub word: String,
}

impl std::fmt::Display for AmbiguityError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "\"{}\" is ambiguous", self.word)
    }
}

impl std::error::Error for AmbiguityError {}
