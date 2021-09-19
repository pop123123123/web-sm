use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Phonem {
    #[serde(rename = "v")]
    pub video_index: u8,
    #[serde(rename = "s")]
    pub start: f64,
    #[serde(rename = "e")]
    pub end: f64,
}

impl PartialEq for Phonem {
    fn eq(&self, other: &Self) -> bool {
        self.video_index == other.video_index && self.start == other.start
    }
}

impl std::hash::Hash for Phonem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.video_index.hash(state);
        ((self.start * 1024.0) as u64).hash(state);
    }
}

pub type ProjectId = String;
pub type Seed = String;
pub type Combo = Vec<Phonem>;
pub type AnalysisResult = Vec<Combo>;

#[derive(Debug)]
pub struct Video {
    pub id: YoutubeId,
    pub asset: ges::UriClipAsset,
    pub lite_asset: ges::UriClipAsset,
}

unsafe impl Send for Video {}

unsafe impl Sync for Video {}

#[derive(Debug, Serialize, Deserialize, Hash, Clone, Eq, PartialEq, Default)]
pub struct YoutubeId {
    pub id: String,
}

impl YoutubeId {
    pub fn new(url: String) -> Self {
        // TODO: Parse url
        YoutubeId { id: url }
    }
}

const VIDEO_PATH: &str = ".videos/";

pub fn get_video_path(yt_id: &str, small: bool) -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(VIDEO_PATH);
    p.push(if small {
        format!("{}_small", yt_id)
    } else {
        yt_id.to_owned()
    });
    p.set_extension("mp4");
    let pwd = std::env::current_dir().unwrap();
    pwd.join(p)
}

impl Video {
    pub fn new(id: YoutubeId) -> Self {
        let in_path = get_video_path(&id.id, false);
        let in_uri = format!("file://{}", in_path.to_str().unwrap());
        let asset = ges::UriClipAsset::request_sync(&in_uri).unwrap();

        let in_path = get_video_path(&id.id, true);
        let in_uri = format!("file://{}", in_path.to_str().unwrap());
        let lite_asset = ges::UriClipAsset::request_sync(&in_uri).unwrap();

        Video {
            id,
            asset,
            lite_asset,
        }
    }

    pub fn get_path_full_resolution(&self) -> std::path::PathBuf {
        get_video_path(&self.id.id, false)
    }

    pub fn get_path_small(&self) -> std::path::PathBuf {
        get_video_path(&self.id.id, true)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub seed: Seed,
    pub video_ids: Vec<YoutubeId>,
    pub name: ProjectId,
    #[serde(skip_serializing)]
    pub segments: Vec<Segment>,
}

impl PartialEq for Project {
    fn eq(&self, other: &Self) -> bool {
        self.seed == other.seed && self.video_ids == other.video_ids
    }
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

impl Project {
    pub fn new(name: &str, seed: &str, video_urls: &[String]) -> Self {
        Project {
            name: name.to_owned(),
            seed: seed.to_owned(),
            video_ids: video_urls
                .iter()
                .map(|u| YoutubeId::new(u.clone()))
                .collect(),
            segments: Default::default(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct AnalysisId(Seed, String, String);
impl AnalysisId {
    pub fn from_project_sentence(project: &Project, sentence: &str) -> AnalysisId {
        AnalysisId(
            project.seed.clone(),
            project
                .video_ids
                .iter()
                .map(|s| &*s.id)
                .collect::<Vec<&str>>()
                .join(""),
            sentence.to_owned(),
        )
    }
}

const PREVIEW_FOLDER: &str = ".preview";
const RENDER_FOLDER: &str = ".render";

#[derive(Debug, PartialEq, Hash)]
pub struct PreviewId<'a>(String, &'a [Phonem]);
impl<'a> PreviewId<'a> {
    pub fn from_project_sentence(yt_ids: &'a [YoutubeId], phonems: &'a [Phonem]) -> Self {
        Self(
            yt_ids
                .iter()
                .map(|s| &*s.id)
                .collect::<Vec<&str>>()
                .join(""),
            phonems,
        )
    }
    fn file_name(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        let hash = hasher.finish();
        hash.to_string()
    }
    pub fn path(&self) -> std::path::PathBuf {
        let mut p = std::env::current_dir().expect("Could not access current directory");
        p.push(PREVIEW_FOLDER);
        p.push(self.file_name());
        p.set_extension("mp4");
        p
    }
    pub fn render_path(&self) -> std::path::PathBuf {
        let mut p = std::env::current_dir().expect("Could not access current directory");
        p.push(RENDER_FOLDER);
        p.push(self.file_name());
        p.set_extension("mp4");
        p
    }
}

impl std::hash::Hash for Project {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.seed.hash(state);
        self.video_ids.hash(state);
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
