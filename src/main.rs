use actix::*;
use actix_files::{Files, NamedFile};
use actix_web::{dev, get, middleware, App, HttpResponse, HttpServer, Responder};
use std::sync::Arc;

mod data;
mod downloader;
mod error;
mod messages;
mod renderer;
mod sm;
mod sm_actor;
mod socket;
mod youtube_dl;

const FRONTEND_PATH: &str = "./front/dist/";
const INDEX_PATH: &str = "./front/dist/index.html";

fn init() {
    ges::init().unwrap();
    std::env::current_dir().unwrap();
}

#[get("/test")]
async fn test() -> impl Responder {
    let p = data::Project::new("lol", "4", &["_ZZ8oyZUGn8".to_string()]);
    let res = sm::analyze(&p, "1 2").await;
    match res {
        Ok(analysis_results) => {
            let phs = &analysis_results[0];
            let vid = crate::data::Video::from(crate::data::YoutubeId {
                id: "_ZZ8oyZUGn8".to_owned(),
            });
            match vid {
                Err(_) => HttpResponse::InternalServerError().finish(),
                Ok(vid) => {
                    let vid = Arc::new(vid);
                    let res = crate::renderer::preview(&[vid], phs);
                    match res {
                        Ok(_) => HttpResponse::Ok().json(&*analysis_results),
                        Err(_) => HttpResponse::InternalServerError().finish(),
                    }
                },
            }
        }
        Err(e) => HttpResponse::BadRequest().json(&e),
    }
}

/// Run actix web server
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3333".to_string())
        .parse()
        .expect("PORT must be a number");
    // Start chat server actor
    let server = sm_actor::SmActor::new().start();

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Compress::default())
            .data(server.clone())
            .service(test)
            .service(actix_web::web::resource("/ws/").to(socket::sm_route))
            .service(
                Files::new("/", FRONTEND_PATH)
                    .index_file("index.html")
                    .default_handler(|req: dev::ServiceRequest| {
                        let (http_req, _payload) = req.into_parts();
                        async {
                            let response = NamedFile::open(INDEX_PATH)?.into_response(&http_req)?;
                            Ok(dev::ServiceResponse::new(http_req, response))
                        }
                    }),
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
