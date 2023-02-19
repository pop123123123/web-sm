use serde::{Deserialize, Serialize};

// #[derive(Debug, Serialize, Deserialize, Display)]
// pub struct ProjectExistsError

// impl std::error::Error for ProjectExistsError {}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerError {
    ProjectDoesNotExist,
    ProjectAlreadyExists,
    EmptyUrls,
    SegmentOutOfBounds,
    UserAlreadyJoinedProject,
    CommunicationError,
}

#[derive(Debug)]
pub enum DownloaderError {
    YoutubeDlCmdNotFoundError,
    DownloadFailedError,
    VideosFolderNotExistError,
    DownloadedVideoNotFoundError,
    RenderingError,
    BrokenRenderedVideo,
}

// impl std::error::Error for ServerError {}
