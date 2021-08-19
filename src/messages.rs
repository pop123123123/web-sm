use crate::data::{Project, ProjectId, Seed, Segment, Video};
use serde::{Deserialize, Serialize};

use crate::sm_actor;

#[derive(Deserialize)]
pub enum ClientRequest {
    ListProjects(sm_actor::ListProjects),
    CreateProject(sm_actor::CreateProject),
    DeleteProject(sm_actor::DeleteProject),
    JoinProject(sm_actor::JoinProject),
    CreateSegment(sm_actor::CreateSegment),
    ModifySegmentSentence(sm_actor::ModifySegmentSentence),
    ModifySegmentComboIndex(sm_actor::ModifySegmentComboIndex),
    RemoveSegment(sm_actor::RemoveSegment),
    Export(sm_actor::Export),
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServerRequest {
    JoinedUsers {
        users: Vec<usize>,
    },
    UserJoinedProject {
        user: usize,
    },
    UserLeftProject {
        user: usize,
    },
    Preview {
        data: String,
        #[serde(flatten)]
        segment: Segment,
    },
    #[serde(rename_all = "camelCase")]
    ChangeProject {
        seed: Seed,
        video_urls: Vec<Video>,
        name: ProjectId,
        segments: Vec<Segment>,
    },
    #[serde(rename_all = "camelCase")]
    NewProject {
        #[serde(flatten)]
        project: Project,
    },
    RemoveProject {
        name: ProjectId,
    },
    NewSegment {
        segment: Segment,
        row: usize,
    },
    RemoveSegment {
        row: usize,
    },
    #[serde(rename_all = "camelCase")]
    ChangeComboIndex {
        row: usize,
        combo_index: u16,
    },
    ChangeSentence {
        row: usize,
        sentence: String,
    },
    ChangeListProjects {
        projects: Vec<Project>,
    },
    RenderResult {
        hash: String,
        data: String,
    },
}
