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
}

// impl std::error::Error for ServerError {}
