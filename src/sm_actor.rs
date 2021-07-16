use crate::data::{Project, ProjectId, Seed, Segment};
use crate::error::*;
use crate::messages::ServerRequest;
use actix::*;
use rand::{self, rngs::ThreadRng, Rng};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

pub type SessionId = usize;
pub type ClientId = usize;

/// Chat server sends this messages to session
#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct SmMessage(pub String);

impl From<&ServerRequest> for SmMessage {
    fn from(request: &ServerRequest) -> SmMessage {
        let data = serde_json::to_string(request).unwrap();
        SmMessage(data)
    }
}

/// New chat session is created
#[derive(Message)]
#[rtype(usize)]
pub struct Connect {
    pub addr: Recipient<SmMessage>,
}

/// Session is disconnected
#[derive(Message, Deserialize)]
#[rtype(result = "()")]
pub struct Disconnect {
    #[serde(skip)]
    pub id: ClientId,
}

/// List of available rooms
#[derive(Deserialize)]
pub struct ListProjects;
impl actix::Message for ListProjects {
    type Result = Vec<Project>;
}

/// Create project and join it
#[derive(Deserialize)]
pub struct CreateProject {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
    pub seed: Seed,
    pub urls: Vec<String>,
}
impl actix::Message for CreateProject {
    type Result = Result<(), ServerError>;
}

/// Delete project and kick all clients who joined it
#[derive(Deserialize)]
pub struct DeleteProject {
    pub project_name: ProjectId,
}
impl actix::Message for DeleteProject {
    type Result = Result<(), ServerError>;
}

/// Join project
#[derive(Deserialize)]
pub struct JoinProject {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
}
impl actix::Message for JoinProject {
    type Result = Result<(), ServerError>;
}

/// Create a segment
#[derive(Deserialize)]
pub struct CreateSegment {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
    pub segment_sentence: String,
    pub position: u16,
}
impl actix::Message for CreateSegment {
    type Result = Result<(), ServerError>;
}

/// Modify a segment's sentence
#[derive(Deserialize)]
pub struct ModifySegmentSentence {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
    pub segment_position: u16,
    pub new_sentence: String,
}
impl actix::Message for ModifySegmentSentence {
    type Result = Result<(), ServerError>;
}

/// Modify a segment's combo index
#[derive(Deserialize)]
pub struct ModifySegmentComboIndex {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
    pub segment_position: u16,
    pub new_combo_index: u16,
}
impl actix::Message for ModifySegmentComboIndex {
    type Result = Result<(), ServerError>;
}

/// Remove a segment
#[derive(Deserialize)]
pub struct RemoveSegment {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
    pub segment_position: u16,
}
impl actix::Message for RemoveSegment {
    type Result = Result<(), ServerError>;
}

pub struct SmActor {
    sessions: HashMap<SessionId, Recipient<SmMessage>>,
    projects: HashMap<ProjectId, Box<Project>>,
    editing_sessions: HashMap<ProjectId, HashSet<ClientId>>,
    rng: ThreadRng,
}

impl SmActor {
    pub fn new() -> SmActor {
        SmActor {
            sessions: HashMap::new(),
            projects: HashMap::new(),
            editing_sessions: HashMap::new(),
            rng: rand::thread_rng(),
        }
    }
}

impl SmActor {
    fn broadcast_except(
        &self,
        project_name: &str,
        user: usize,
        request: &ServerRequest,
    ) -> Result<(), ServerError> {
        let m = SmMessage::from(request);
        self.editing_sessions[project_name]
            .iter()
            .filter(|id_| **id_ != user)
            .for_each(|id| {
                self.sessions[id].do_send(m.clone());
            });
        Ok(())
    }
    fn broadcast(&self, project_name: &str, request: &ServerRequest) -> Result<(), ServerError> {
        let m = SmMessage::from(request);
        self.editing_sessions[project_name].iter().for_each(|id| {
            self.sessions[id].do_send(m.clone());
        });
        Ok(())
    }
    fn send(&self, user: usize, request: &ServerRequest) -> Result<(), ServerError> {
        let m = SmMessage::from(request);
        self.sessions[&user]
            .do_send(m)
            .map_err(|_| ServerError::CommunicationError)
    }

    fn create_project(
        &mut self,
        project_name: ProjectId,
        seed: Seed,
        video_urls: &[String],
    ) -> Result<Box<Project>, ServerError> {
        if self.projects.contains_key(&project_name) {
            return Err(ServerError::ProjectAlreadyExists);
        }

        let project = Box::new(Project::new(&project_name, &seed, video_urls));
        self.projects.insert(project_name.clone(), project.clone());
        self.editing_sessions.insert(project_name, HashSet::new());

        Ok(project)
    }
    fn delete_project(&mut self, project_name: ProjectId) -> Result<(), ServerError> {
        if !self.projects.contains_key(&project_name) {
            return Err(ServerError::ProjectDoesNotExist);
        }
        // TODO: notify connected users

        self.projects.remove(&project_name);
        Ok(())
    }

    fn user_join_project(
        &mut self,
        project_name: ProjectId,
        user: ClientId,
    ) -> Result<(), ServerError> {
        let users = &self.editing_sessions[&project_name];
        if users.contains(&user) {
            return Err(ServerError::UserAlreadyJoinedProject);
        }

        let users = self
            .editing_sessions
            .get_mut(&project_name)
            .expect("Inconsistent sessions/data");
        users.insert(user);

        let Project {
            seed,
            video_urls,
            name,
            segments,
        } = &*self.projects[&project_name];
        let r = ServerRequest::ChangeProject {
            seed: (*seed).clone(),
            video_urls: (*video_urls).clone(),
            name: (*name).clone(),
            segments: (*segments).clone(),
        };
        self.send(user, &r).ok();

        let r = ServerRequest::UserJoinedProject { user };
        self.broadcast_except(&project_name, user, &r)
    }

    fn add_segment(
        &mut self,
        project_name: ProjectId,
        position: u16,
        sentence: String,
    ) -> Result<(), ServerError> {
        let project = match self.projects.get_mut(&project_name) {
            Some(p) => p,
            None => return Err(ServerError::ProjectDoesNotExist),
        };

        if position as usize > project.segments.len() {
            return Err(ServerError::SegmentOutOfBounds);
        }

        let segment = Segment::new(&sentence);
        project.segments.insert(position as usize, segment.clone());

        // TODO: run analysis

        let r = ServerRequest::NewSegment {
            segment,
            row: position as usize,
        };
        self.broadcast(&project_name, &r)
    }

    fn modify_segment_sentence(
        &mut self,
        project_name: ProjectId,
        segment_position: u16,
        sentence: String,
    ) -> Result<(), ServerError> {
        let project = match self.projects.get_mut(&project_name) {
            Some(p) => p,
            None => return Err(ServerError::ProjectDoesNotExist),
        };

        let segment = match project.segments.get_mut(segment_position as usize) {
            Some(s) => s,
            None => return Err(ServerError::SegmentOutOfBounds),
        };
        segment.sentence = sentence.clone();

        let r = ServerRequest::ChangeSentence {
            row: segment_position as usize,
            sentence,
        };
        self.broadcast(&project_name, &r)
    }

    fn modify_segment_combo_index(
        &mut self,
        project_name: ProjectId,
        segment_position: u16,
        index: u16,
    ) -> Result<(), ServerError> {
        let project = match self.projects.get_mut(&project_name) {
            Some(p) => p,
            None => return Err(ServerError::ProjectDoesNotExist),
        };

        let segment = match project.segments.get_mut(segment_position as usize) {
            Some(s) => s,
            None => return Err(ServerError::SegmentOutOfBounds),
        };
        segment.combo_index = index;

        let r = ServerRequest::ChangeComboIndex {
            row: segment_position as usize,
            combo_index: index,
        };
        self.broadcast(&project_name, &r)
    }

    fn remove_segment(
        &mut self,
        project_name: ProjectId,
        segment_position: u16,
    ) -> Result<(), ServerError> {
        let project = match self.projects.get_mut(&project_name) {
            Some(p) => p,
            None => return Err(ServerError::ProjectDoesNotExist),
        };

        if segment_position as usize >= project.segments.len() {
            return Err(ServerError::SegmentOutOfBounds);
        }

        project.segments.remove(segment_position as usize);

        let r = ServerRequest::RemoveSegment {
            row: segment_position as usize,
        };
        self.broadcast(&project_name, &r)
    }
}

impl Actor for SmActor {
    type Context = Context<Self>;
}

/// Register new session and assign unique id to this session
impl Handler<Connect> for SmActor {
    type Result = usize;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        println!("Someone joined");

        // register session with random id
        let id = self.rng.gen::<SessionId>();
        self.sessions.insert(id, msg.addr);

        // send id back
        id
    }
}

/// Handler for Disconnect message.
impl Handler<Disconnect> for SmActor {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        let m = SmMessage(format!("{} disconnected", &msg.id));
        // Removing client from all subscribed sessions
        let rooms: Vec<_> = self
            .editing_sessions
            .values_mut()
            .map(|s| {
                s.remove(&msg.id);
                s
            })
            .collect();
        rooms
            .iter()
            .fold(HashSet::new(), |acc, hs| acc.union(hs).cloned().collect())
            .iter()
            .for_each(|id| {
                self.sessions[&id].do_send(m.clone()).ok();
            });
        self.sessions.remove(&msg.id);
        // TODO: notify other users that this one left

        // TODO: free projects that are not edited anymore by anybody
    }
}

/// Handler for `ListProjects` message.
impl Handler<ListProjects> for SmActor {
    type Result = MessageResult<ListProjects>;

    fn handle(&mut self, _: ListProjects, _: &mut Context<Self>) -> Self::Result {
        let projects = self.projects.values().map(|p| (**p).clone()).collect();
        MessageResult(projects)
    }
}

// Creates a project and joins it automatically
impl Handler<CreateProject> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: CreateProject, _: &mut Context<Self>) -> Self::Result {
        let CreateProject {
            id,
            project_name,
            seed,
            urls,
        } = msg;

        println!("New project: {} {} {:?}", project_name, seed, urls);
        self.create_project(project_name.clone(), seed, &urls)
            .map(|_| ())?;
        self.user_join_project(project_name, id)?;
        Ok(())
    }
}

// Creates a project and joins it automatically
impl Handler<DeleteProject> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: DeleteProject, _: &mut Context<Self>) -> Self::Result {
        let DeleteProject { project_name } = msg;

        self.delete_project(project_name)
    }
}

// Joins a project
impl Handler<JoinProject> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: JoinProject, _: &mut Context<Self>) -> Self::Result {
        let JoinProject { id, project_name } = msg;

        self.user_join_project(project_name, id)?;
        Ok(())
    }
}

// Creates a segment
impl Handler<CreateSegment> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: CreateSegment, _: &mut Context<Self>) -> Self::Result {
        let CreateSegment {
            project_name,
            segment_sentence,
            position,
            ..
        } = msg;

        self.add_segment(project_name, position, segment_sentence)
    }
}

// Modifies segment sentence
impl Handler<ModifySegmentSentence> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: ModifySegmentSentence, _: &mut Context<Self>) -> Self::Result {
        let ModifySegmentSentence {
            project_name,
            segment_position,
            new_sentence,
            ..
        } = msg;

        self.modify_segment_sentence(project_name, segment_position, new_sentence)
    }
}

// Modifies segment combo index
impl Handler<ModifySegmentComboIndex> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: ModifySegmentComboIndex, _: &mut Context<Self>) -> Self::Result {
        let ModifySegmentComboIndex {
            project_name,
            segment_position,
            new_combo_index,
            ..
        } = msg;

        self.modify_segment_combo_index(project_name, segment_position, new_combo_index)
    }
}

// Removes a segment
impl Handler<RemoveSegment> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: RemoveSegment, _: &mut Context<Self>) -> Self::Result {
        let RemoveSegment {
            project_name,
            segment_position,
            ..
        } = msg;

        self.remove_segment(project_name, segment_position)
    }
}
