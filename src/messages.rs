use serde::Deserialize;

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
}
