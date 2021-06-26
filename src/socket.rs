use std::time::{Duration, Instant};

use actix::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;

use crate::messages::ClientRequest;
use crate::sm_actor;

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Entry point for our websocket route
pub async fn sm_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<sm_actor::SmActor>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        WsSmSession {
            id: 0,
            hb: Instant::now(),
            addr: srv.get_ref().clone(),
        },
        &req,
        stream,
    )
}

struct WsSmSession {
    /// unique session id
    id: sm_actor::ClientId,
    /// Client must send ping at least once per 10 seconds (CLIENT_TIMEOUT),
    /// otherwise we drop connection.
    hb: Instant,
    /// SM server
    addr: Addr<sm_actor::SmActor>,
}

impl Actor for WsSmSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        let addr: Addr<WsSmSession> = ctx.address();
        // Trying to get a session ID
        self.addr
            .send(sm_actor::Connect {
                addr: addr.recipient(),
            })
            // Get the response
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.id = res,
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.addr.do_send(sm_actor::Disconnect { id: self.id });
        Running::Stop
    }
}

//WebSocket message handler
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSmSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(_) => {
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        println!("WEBSOCKET MESSAGE: {:?}", msg);

        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => match serde_json::from_str(&text) {
                Ok(ClientRequest::ListProjects) => {
                    self.addr
                        .send(sm_actor::ListProjects)
                        .into_actor(self)
                        .then(|res, _, ctx| {
                            ctx.text(serde_json::to_string(&res.unwrap()).unwrap());
                            fut::ready(())
                        })
                        .wait(ctx);
                }
                Ok(ClientRequest::CreateProject {
                    project_name,
                    seed,
                    urls,
                }) => {
                    let req = sm_actor::CreateProject {
                        id: self.id,
                        seed,
                        project_name,
                        urls,
                    };
                    self.addr
                        .send(req)
                        .into_actor(self)
                        .then(|res, _, ctx| {
                            ctx.text(serde_json::to_string(&res.unwrap()).unwrap());
                            fut::ready(())
                        })
                        .wait(ctx);
                }
                Ok(ClientRequest::DeleteProject { project_name }) => {
                    let req = sm_actor::DeleteProject { project_name };
                    self.addr
                        .send(req)
                        .into_actor(self)
                        .then(|res, _, ctx| {
                            ctx.text(serde_json::to_string(&res.unwrap()).unwrap());
                            fut::ready(())
                        })
                        .wait(ctx);
                }
                _ => {
                    println!("unrecognized request")
                }
            },
            ws::Message::Binary(_) => println!("Lol"),
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Nop => (),
            _ => (),
        }
    }
}

impl WsSmSession {
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!("Websocket Client heartbeat failed, disconnecting!");
                act.addr.do_send(sm_actor::Disconnect { id: act.id });
                ctx.stop();
                return;
            }

            ctx.ping(b"");
        });
    }
}

impl Handler<sm_actor::SmMessage> for WsSmSession {
    type Result = ();

    fn handle(&mut self, _msg: sm_actor::SmMessage, _: &mut Self::Context) -> Self::Result {}
}

impl Handler<sm_actor::Connect> for WsSmSession {
    type Result = usize;

    fn handle(&mut self, _msg: sm_actor::Connect, _: &mut Self::Context) -> Self::Result {
        1
    }
}
