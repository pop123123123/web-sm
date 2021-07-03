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

pub type ProjectId = String;
pub type Seed = String;
pub type Combo = Vec<Phonem>;
pub type AnalysisResult = Vec<Combo>;

#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct Project {
    pub seed: Seed,
    pub video_urls: Vec<String>,
    pub name: ProjectId,
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct Segment {
    #[serde(rename = "s")]
    pub sentence: String,
    #[serde(rename = "i")]
    pub combo_index: u16,
}

impl Segment {
    pub fn new(sentence: &str) -> Self {
        Segment {
            sentence: sentence.to_owned(),
            combo_index: 0,
        }
    }
}

fn transform_url(url: &str) -> &str {
    url
}

impl Project {
    pub fn new(name: &str, seed: &str, video_urls: &[String]) -> Self {
        Project {
            name: name.to_owned(),
            seed: seed.to_owned(),
            video_urls: video_urls
                .iter()
                .map(|u| transform_url(u).to_owned())
                .collect(),
            segments: Default::default(),
        }
    }
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
