use crate::data::{Project, ProjectId, Seed, Segment};
use crate::error::*;
use actix::*;
use rand::{self, rngs::ThreadRng, Rng};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

pub type SessionId = usize;
pub type ClientId = usize;

/// Chat server sends this messages to session
#[derive(Message)]
#[rtype(result = "()")]
pub struct SmMessage(pub String);

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
#[derive(Message, Deserialize)]
#[rtype(result = "()")]
pub struct JoinProject {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
}

/// Create a segment
#[derive(Message, Deserialize)]
#[rtype(result = "()")]
pub struct CreateSegment {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
    pub segment_sentence: String,
    pub position: u16,
}

/// Modify a segment's sentence
#[derive(Message, Deserialize)]
#[rtype(result = "()")]
pub struct ModifySegmentSentence {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
    pub segment_position: u16,
    pub new_sentence: String,
}

/// Modify a segment's combo index
#[derive(Message, Deserialize)]
#[rtype(result = "()")]
pub struct ModifySegmentComboIndex {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
    pub segment_position: u16,
    pub new_combo_index: u16,
}

/// Remove a segment
#[derive(Message, Deserialize)]
#[rtype(result = "()")]
pub struct RemoveSegment {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
    pub segment_position: u16,
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
        self.projects.insert(project_name, project.clone());

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
        let users_result = self.editing_sessions.get_mut(&project_name);

        match users_result {
            Some(users) => {
                // User already on the project
                // Add new user to the has set
                if users.iter().any(|id| *id == user) {
                    return Err(ServerError::UserAlreadyJoinedProject);
                }
                users.insert(user);
                Ok(())
            }
            None => {
                // No user on this projects yet
                // Creates new hash set containing the user
                let mut users = HashSet::new();
                users.insert(user);
                self.editing_sessions.insert(project_name, users);
                Ok(())
            }
        }
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

        if position as usize >= project.segments.len() {
            return Err(ServerError::SegmentOutOfBounds);
        }

        project
            .segments
            .insert(position as usize, Segment::new(&sentence));

        // TODO: run analysis

        Ok(())
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
        segment.sentence = sentence;
        Ok(())
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
        Ok(())
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

        Ok(())
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
        // Removing client from all subscribed sessions
        self.editing_sessions.values_mut().for_each(|s| {
            s.remove(&msg.id);
        });
        // TODO: alert other clients
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
            id: _,
            project_name,
            seed,
            urls,
        } = msg;

        println!("creates new project");

        self.create_project(project_name, seed, &urls).map(|_| ())

        // if self.user_join_project(project_name, id).is_err() {
        //     todo!("Return error to socket");
        // }
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
    type Result = ();

    fn handle(&mut self, msg: JoinProject, _: &mut Context<Self>) {
        let JoinProject { id, project_name } = msg;

        if self.user_join_project(project_name, id).is_err() {
            todo!("Return error to socket");
        }
    }
}

// Creates a segment
impl Handler<CreateSegment> for SmActor {
    type Result = ();

    fn handle(&mut self, msg: CreateSegment, _: &mut Context<Self>) {
        let CreateSegment {
            project_name,
            segment_sentence,
            position,
            ..
        } = msg;

        if self
            .add_segment(project_name, position, segment_sentence)
            .is_err()
        {
            todo!("Return error to socket");
        }
    }
}

// Modifies segment sentence
impl Handler<ModifySegmentSentence> for SmActor {
    type Result = ();

    fn handle(&mut self, msg: ModifySegmentSentence, _: &mut Context<Self>) {
        let ModifySegmentSentence {
            project_name,
            segment_position,
            new_sentence,
            ..
        } = msg;

        if self
            .modify_segment_sentence(project_name, segment_position, new_sentence)
            .is_err()
        {
            todo!("Return error to socket");
        }
    }
}

// Modifies segment combo index
impl Handler<ModifySegmentComboIndex> for SmActor {
    type Result = ();

    fn handle(&mut self, msg: ModifySegmentComboIndex, _: &mut Context<Self>) {
        let ModifySegmentComboIndex {
            project_name,
            segment_position,
            new_combo_index,
            ..
        } = msg;

        if self
            .modify_segment_combo_index(project_name, segment_position, new_combo_index)
            .is_err()
        {
            todo!("Return error to socket");
        }
    }
}

// Removes a segment
impl Handler<RemoveSegment> for SmActor {
    type Result = ();

    fn handle(&mut self, msg: RemoveSegment, _: &mut Context<Self>) {
        let RemoveSegment {
            project_name,
            segment_position,
            ..
        } = msg;

        if self.remove_segment(project_name, segment_position).is_err() {
            todo!("Return error to socket");
        }
    }
}
