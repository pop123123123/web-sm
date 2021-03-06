use crate::data::PreviewId;
use crate::data::{Preview, Project, ProjectId, Seed, Segment};
use crate::downloader::GetVideos;
use crate::error::*;
use crate::messages::ServerRequest;
use actix::*;
use rand::{self, rngs::ThreadRng, Rng};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

pub type SessionId = usize;
pub type ClientId = usize;

use crate::sm;

macro_rules! clone_project {
    ($self:expr, $project_name:expr) => {{
        (**$self.projects.get(&$project_name).unwrap()).clone()
    }};
}

macro_rules! clone_segment {
    ($self:expr, $project_name:expr, $position:expr) => {{
        $self.projects[&$project_name].segments[$position as usize].clone()
    }};
}

macro_rules! async_run_preview {
    ($request:expr, $recipients:expr, $project:expr, $segment:expr, $fut_videos: expr, $preview: expr, $segment_position: expr) => {
        async move {
            broadcast($request, &$recipients).await;
            // Prepare preview and sends it
            if $preview {
                let combos = sm::analyze(&$project, &$segment.sentence).await;
                if let Err(ambiguity) = combos {
                    let request = ServerRequest::AmbiguityToken {
                        token: ambiguity.word,
                        row: $segment_position,
                    };
                    broadcast(request, &$recipients).await;
                    return;
                }
                let combos = combos.unwrap();

                let videos = $fut_videos.await;
                if let Err(_) = videos {
                    // Mailbox is full and we should just ignore this
                    return;
                }
                let videos = videos.unwrap();

                if let Err(_) = videos {
                    println!("Video downloading is pending, cannot generate the preview yet");
                    // TODO: We should just ignore and wait
                    // Maybe send a message to the client to notify that
                    return;
                }
                let videos = videos.unwrap();

                // TODO: run n first previews
                let res = crate::renderer::preview(&videos, &combos[$segment.combo_index as usize]);

                if let Err(_) = res {
                    println!("Error while generating the preview");
                    // TODO: We should probably retry
                    return;
                }
                let path = res.unwrap();

                let bytes = async_fs::read(path).await;
                if let Err(_) = bytes {
                    println!("Cannot find preview in filesystem");
                    // TODO: We should probably re-compute the preview
                    return;
                }
                let bytes = bytes.unwrap();

                let decoder = base64::encode(bytes);
                let data = decoder.to_owned();
                let r = ServerRequest::Preview {
                    segment: $segment,
                    data,
                };
                broadcast(r, &$recipients).await;
            }
        }
    };
}

macro_rules! send_broadcast_async_preview {
    ($self:expr, $project_name: expr, $segment_position: expr, $request: expr, $preview: expr) => {{
        let recipients = $self.get_all_cloned_recipients_project(&$project_name);

        let segment = clone_segment!($self, $project_name, $segment_position);
        let project = clone_project!($self, $project_name);

        let fut_videos = $self.downloader.send(GetVideos {
            yt_ids: project.video_ids.clone(),
        });

        async_run_preview!(
            $request,
            recipients,
            project,
            segment,
            fut_videos,
            $preview,
            $segment_position
        )
    }};
}

/// Chat server sends this messages to session
#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct SmMessage(pub String);

impl From<&ServerRequest> for SmMessage {
    fn from(request: &ServerRequest) -> SmMessage {
        let data = serde_json::to_string(request).unwrap(); // Good
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
    type Result = ServerRequest;
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

/// Remove a segment
#[derive(Deserialize)]
pub struct Export {
    #[serde(skip)]
    pub id: ClientId,
    pub project_name: ProjectId,
}
impl actix::Message for Export {
    type Result = Result<(), ServerError>;
}

/// Load a project
#[derive(Deserialize)]
pub struct Load {
    pub project: Project,
}
impl actix::Message for Load {
    type Result = Result<(), ServerError>;
}

pub struct SmActor {
    sessions: HashMap<SessionId, Recipient<SmMessage>>,
    projects: HashMap<ProjectId, Box<Project>>,
    editing_sessions: HashMap<ProjectId, HashSet<ClientId>>,
    rng: ThreadRng,
    downloader: actix::Addr<crate::downloader::DownloaderActor>,
}

impl SmActor {
    pub fn new() -> SmActor {
        SmActor {
            sessions: HashMap::new(),
            projects: HashMap::new(),
            editing_sessions: HashMap::new(),
            rng: rand::thread_rng(),
            downloader: crate::downloader::DownloaderActor::new().start(),
        }
    }
}

fn hash_segments(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| s.sentence.clone() + &s.combo_index.to_string())
        .collect::<Vec<_>>()
        .join("")
}

// Async function used to send a server request to a list of recipients
async fn broadcast(request: ServerRequest, recipients: &[Recipient<SmMessage>]) {
    let m = SmMessage::from(&request);
    let future_send = recipients.iter().map(|recipient| {
        //TODO: check send Result
        recipient.send(m.clone())
    });
    futures::future::join_all(future_send).await;
}

impl SmActor {
    fn get_all_recipients(&self) -> Vec<Recipient<SmMessage>> {
        let recipients: Vec<_> = self.sessions.values().cloned().collect();
        recipients
    }

    fn get_all_cloned_recipients_project(&self, project_name: &str) -> Vec<Recipient<SmMessage>> {
        // Get the list of the sessions linked to the project
        let recipients: Vec<_> = self.editing_sessions[project_name]
            .iter()
            .map(|id| self.sessions[id].clone())
            .collect();

        recipients
    }

    fn get_all_cloned_recipients_project_except(
        &self,
        project_name: &str,
        user: usize,
    ) -> Vec<Recipient<SmMessage>> {
        let recipients: Vec<_> = self.editing_sessions[project_name]
            .iter()
            .filter(|id_| **id_ != user)
            .map(|id| self.sessions[id].clone())
            .collect();

        recipients
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
    fn delete_project(&mut self, project_name: ProjectId) -> Result<ServerRequest, ServerError> {
        if !self.projects.contains_key(&project_name) {
            return Err(ServerError::ProjectDoesNotExist);
        }

        self.projects.remove(&project_name);

        let r = ServerRequest::RemoveProject { name: project_name };

        Ok(r)
    }

    fn user_join_project(
        &mut self,
        project_name: ProjectId,
        user: ClientId,
    ) -> Result<(ServerRequest, ServerRequest, ServerRequest), ServerError> {
        // Check if project exists
        if self.projects.get(&project_name.clone()).is_none() {
            return Err(ServerError::ProjectDoesNotExist);
        }

        let users = &self.editing_sessions[&project_name];
        if users.contains(&user) {
            return Err(ServerError::UserAlreadyJoinedProject);
        }

        let users = self
            .editing_sessions
            .get_mut(&project_name)
            .expect("Inconsistent sessions/data");
        let request_joined_users = ServerRequest::JoinedUsers {
            users: users.iter().copied().collect(),
        };
        users.insert(user);

        let Project {
            seed,
            video_ids,
            name,
            segments,
        } = &*self.projects[&project_name];
        let request_user_change_server = ServerRequest::ChangeProject {
            seed: (*seed).clone(),
            video_urls: (*video_ids).clone(),
            name: (*name).clone(),
            segments: (*segments).clone(),
        };
        let request_notify_join = ServerRequest::UserJoinedProject { user };

        Ok((
            request_joined_users,
            request_user_change_server,
            request_notify_join,
        ))
    }

    fn add_segment(
        &mut self,
        project_name: ProjectId,
        position: u16,
        sentence: String,
    ) -> Result<ServerRequest, ServerError> {
        let project = match self.projects.get_mut(&project_name) {
            Some(p) => p,
            None => return Err(ServerError::ProjectDoesNotExist),
        };

        if position as usize > project.segments.len() {
            return Err(ServerError::SegmentOutOfBounds);
        }

        let segment = Segment::new(&sentence);
        project.segments.insert(position as usize, segment.clone());

        let r = ServerRequest::NewSegment {
            segment,
            row: position as usize,
        };

        Ok(r)
    }

    fn modify_segment_sentence(
        &mut self,
        project_name: ProjectId,
        segment_position: u16,
        sentence: String,
    ) -> Result<ServerRequest, ServerError> {
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

        Ok(r)
    }

    fn modify_segment_combo_index(
        &mut self,
        project_name: ProjectId,
        segment_position: u16,
        index: u16,
    ) -> Result<ServerRequest, ServerError> {
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
        Ok(r)
    }

    fn remove_segment(
        &mut self,
        project_name: ProjectId,
        segment_position: u16,
    ) -> Result<ServerRequest, ServerError> {
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
        Ok(r)
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

    fn handle(&mut self, msg: Disconnect, ctx: &mut Context<Self>) {
        let r = ServerRequest::UserLeftProject { user: msg.id };
        // Removing client from all subscribed sessions
        let rooms: Vec<_> = self
            .editing_sessions
            .values_mut()
            .map(|s| {
                s.remove(&msg.id);
                s
            })
            .collect();

        let recipients: Vec<_> = rooms
            .iter()
            .fold(HashSet::new(), |acc, hs| acc.union(hs).cloned().collect())
            .iter()
            .map(|id| self.sessions[&id].clone())
            .collect();

        self.sessions.remove(&msg.id);

        let fut = async move {
            broadcast(r, &recipients).await;
        };

        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);

        // TODO: free projects that are not edited anymore by anybody
    }
}

/// Handler for `ListProjects` message.
impl Handler<ListProjects> for SmActor {
    type Result = MessageResult<ListProjects>;

    fn handle(&mut self, _: ListProjects, _: &mut Context<Self>) -> Self::Result {
        let projects: Vec<_> = self.projects.values().map(|p| (**p).clone()).collect();

        MessageResult(ServerRequest::ChangeListProjects { projects })
    }
}

// Creates a project and joins it automatically
impl Handler<CreateProject> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: CreateProject, ctx: &mut Context<Self>) -> Self::Result {
        let CreateProject {
            id,
            project_name,
            seed,
            urls,
        } = msg;

        // Creating a new project
        println!("New project: {} {} {:?}", project_name, seed, urls);
        let project = self.create_project(project_name.clone(), seed, &urls)?;

        let project_yt_ids = project.video_ids.to_vec();

        let all_recipients = self.get_all_recipients();

        let new_project_request = ServerRequest::NewProject {
            project: (*project),
        };

        // Adding user to it
        let (request_joined_users, request_user_change_server, request_notify_join) =
            self.user_join_project(project_name.clone(), id)?;

        let user_recipient_clone = self.sessions[&id].clone();
        let all_recipients_except =
            self.get_all_cloned_recipients_project_except(&project_name, id);

        let msg = crate::downloader::DownloadVideos {
            yt_ids: project_yt_ids,
        };
        let send_download_message = self.downloader.send(msg);

        let fut = async move {
            // Notify all users that a project have been created
            broadcast(new_project_request, &all_recipients).await;

            // Adding user to the project and notify all the other users on the project
            user_join_project_async(
                request_joined_users,
                request_user_change_server,
                request_notify_join,
                user_recipient_clone,
                all_recipients_except,
            )
            .await;

            // unwrap is safe, because sending to a local actor can not fail
            let dl = send_download_message.await.unwrap();
            if let Err(DownloaderError::YoutubeDlCmdNotFoundError) = dl {
                println!("Could not find youtube-dl bin");
            } else if let Err(DownloaderError::DownloadFailedError) = dl {
                println!("Failed to download the videos");
            }
        };

        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);

        Ok(())
    }
}

// Creates a project and joins it automatically
impl Handler<DeleteProject> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: DeleteProject, ctx: &mut Context<Self>) -> Self::Result {
        let DeleteProject { project_name } = msg;

        let request = match self.delete_project(project_name.clone()) {
            Ok(r) => r,
            Err(e) => return Err(e),
        };

        let recipients = self.get_all_cloned_recipients_project(&project_name);

        let fut = async move {
            broadcast(request, &recipients).await;
        };

        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);

        Ok(())
    }
}

async fn user_join_project_async(
    request_joined_users: ServerRequest,
    request_user_change_server: ServerRequest,
    request_notify_join: ServerRequest,
    user_recipient_clone: Recipient<SmMessage>,
    all_recipients_except: Vec<Recipient<SmMessage>>,
) {
    // Send the list of joined users to the user
    let m = SmMessage::from(&request_joined_users);
    user_recipient_clone.send(m).await.ok();

    // Add user on the project
    let m = SmMessage::from(&request_user_change_server);
    user_recipient_clone.send(m).await.ok();

    // Notify all the other project's users
    broadcast(request_notify_join, &all_recipients_except).await;
}

// Joins a project
impl Handler<JoinProject> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: JoinProject, ctx: &mut Context<Self>) -> Self::Result {
        let JoinProject { id, project_name } = msg;

        let (request_joined_users, request_user_change_server, request_notify_join) =
            self.user_join_project(project_name.clone(), id)?;

        let user_recipient_clone = self.sessions[&id].clone();
        let all_recipients_except =
            self.get_all_cloned_recipients_project_except(&project_name.clone(), id);

        let project = clone_project!(self, project_name.clone());

        let previews_fut: Vec<_> = project
            .segments
            .iter()
            .map(|segment| {
                let project = project.clone();
                let segment = segment.clone();
                async move {
                    let combos = sm::analyze(&project, &segment.sentence).await;
                    if let Err(_) = combos {
                        return None;
                    }
                    let combos = combos.unwrap();

                    let preview = PreviewId::from_project_sentence(
                        &project.video_ids,
                        &combos[segment.combo_index as usize],
                    );
                    let path = preview.path();

                    let bytes = async_fs::read(path).await;
                    if let Err(_) = bytes {
                        return None;
                    }
                    let bytes = bytes.unwrap();

                    let decoder = base64::encode(bytes);
                    let data = decoder;

                    Some(Preview {
                        data: data,
                        segment: segment,
                    })
                }
            })
            .collect();
        let fut = async move {
            user_join_project_async(
                request_joined_users,
                request_user_change_server,
                request_notify_join,
                user_recipient_clone.clone(),
                all_recipients_except,
            )
            .await;

            let all_previews = futures::future::join_all(previews_fut).await;
            let all_previews = all_previews
                .into_iter()
                .filter(|preview| (*preview).is_some())
                .map(|preview| preview.unwrap())
                .collect::<Vec<_>>();

            let r = ServerRequest::Previews {
                previews: all_previews,
            };
            if user_recipient_clone
                .send(SmMessage::from(&r))
                .await
                .is_err()
            {
                println!("All previews message not properly sent");
            }
        };

        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);

        Ok(())
    }
}

// Creates a segment
impl Handler<CreateSegment> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: CreateSegment, ctx: &mut Context<Self>) -> Self::Result {
        let CreateSegment {
            project_name,
            segment_sentence,
            position,
            ..
        } = msg;

        let request =
            match self.add_segment(project_name.clone(), position, segment_sentence.clone()) {
                Ok(r) => r,
                Err(e) => return Err(e),
            };

        let fut = send_broadcast_async_preview!(
            self,
            project_name,
            position as usize,
            request,
            !segment_sentence.trim().is_empty()
        );

        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);

        Ok(())
    }
}

// Modifies segment sentence
impl Handler<ModifySegmentSentence> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: ModifySegmentSentence, ctx: &mut Context<Self>) -> Self::Result {
        let ModifySegmentSentence {
            project_name,
            segment_position,
            new_sentence,
            ..
        } = msg;

        // Retrieve a server request
        let request = match self.modify_segment_sentence(
            project_name.clone(),
            segment_position,
            new_sentence.clone(),
        ) {
            Ok(r) => r,
            Err(e) => return Err(e),
        };

        let fut = send_broadcast_async_preview!(
            self,
            project_name,
            segment_position as usize,
            request,
            true
        );
        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);

        Ok(())
    }
}

// Modifies segment combo index
impl Handler<ModifySegmentComboIndex> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: ModifySegmentComboIndex, ctx: &mut Context<Self>) -> Self::Result {
        let ModifySegmentComboIndex {
            project_name,
            segment_position,
            new_combo_index,
            ..
        } = msg;

        let request = match self.modify_segment_combo_index(
            project_name.clone(),
            segment_position,
            new_combo_index,
        ) {
            Ok(r) => r,
            Err(e) => return Err(e),
        };

        let fut = send_broadcast_async_preview!(
            self,
            project_name,
            segment_position as usize,
            request,
            true
        );
        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);

        Ok(())
    }
}

// Removes a segment
impl Handler<RemoveSegment> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: RemoveSegment, ctx: &mut Context<Self>) -> Self::Result {
        let RemoveSegment {
            project_name,
            segment_position,
            ..
        } = msg;

        // Retrieve a server request
        let request = match self.remove_segment(project_name.clone(), segment_position) {
            Ok(r) => r,
            Err(e) => return Err(e),
        };

        // Get the list of the sessions linked to the project
        let recipients = self.get_all_cloned_recipients_project(&project_name);

        let fut = async move {
            // Send the notification to all involved sessions
            broadcast(request, &recipients).await;
        };

        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);

        Ok(())
    }
}
// Removes a segment
impl Handler<Export> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: Export, ctx: &mut Context<Self>) -> Self::Result {
        let Export { project_name, .. } = msg;

        // Get the list of the sessions linked to the project
        let recipients = self.get_all_cloned_recipients_project(&project_name);

        let project = clone_project!(self, project_name);

        let fut_videos = self.downloader.send(GetVideos {
            yt_ids: project.video_ids.clone(),
        });

        let fut = async move {
            // Prepare preview and sends it
            let analysis = project
                .segments
                .iter()
                .map(|s| sm::analyze(&project, &s.sentence));
            let res: Vec<_> = futures::future::join_all(analysis).await;
            // Throw away all failed analysis (because of ambiguities) and keep all the others
            let res: Vec<_> = res
                .iter()
                .filter(|c| c.as_ref().is_ok())
                .map(|c| c.as_ref().unwrap())
                .collect();

            let combos: Vec<_> = res
                .iter()
                .enumerate()
                .map(|(i, combos)| {
                    combos[project.segments[i as usize].combo_index as usize]
                        .iter()
                        .map(|p| (*p).clone())
                })
                .flatten()
                .collect();

            let video_res = fut_videos.await;
            if let Err(_) = video_res {
                println!("Mailbox full. Cannot export");
                // TODO: mailbox full. What should we do ?
                return;
            }
            let video_res = video_res.unwrap();

            if let Err(_) = video_res {
                println!("Video downloading is pending, cannot generate the preview yet");
                // TODO: We should just ignore and wait
                // Maybe send a message to the client to notify that
                return;
            }
            let videos = video_res.unwrap();

            let rendering = crate::renderer::render(&videos, &combos);
            if let Err(_) = rendering {
                println!("Error while generating the rendered video");
                // TODO: We should notify the user
                return;
            }
            let path = rendering.unwrap();

            let bytes = async_fs::read(path).await;
            if let Err(_) = bytes {
                println!("Cannot find rendered video in filesystem");
                // TODO: We should notify the user
                return;
            }
            let bytes = bytes.unwrap();

            let decoder = base64::encode(bytes);
            let data = decoder.to_owned();
            let hash = hash_segments(&project.segments);
            let r = ServerRequest::RenderResult { hash, data };
            broadcast(r, &recipients).await;
        };

        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);

        Ok(())
    }
}

// Load a project
impl Handler<Load> for SmActor {
    type Result = Result<(), ServerError>;

    fn handle(&mut self, msg: Load, ctx: &mut Context<Self>) -> Self::Result {
        if self.projects.contains_key(&msg.project.name) {
            return Err(ServerError::ProjectAlreadyExists);
        }
        self.projects
            .insert(msg.project.name.clone(), Box::new(msg.project.clone()));

        let new_project_request = ServerRequest::NewProject {
            project: msg.project,
        };

        let all_recipients = self.get_all_recipients();
        let fut = async move {
            broadcast(new_project_request, &all_recipients).await;
        };
        let fut = actix::fut::wrap_future::<_, Self>(fut);
        ctx.spawn(fut);

        Ok(())
    }
}
