use serde::Deserialize;

use crate::data::{ProjectId, Seed};

#[derive(Deserialize)]
pub enum ClientRequest {
    ListProjects,
    CreateProject {
        project_name: ProjectId,
        seed: Seed,
        urls: Vec<String>,
    },
    DeleteProject {
        project_name: ProjectId,
    },
}
