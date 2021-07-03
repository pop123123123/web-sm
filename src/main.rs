use actix::*;
use actix_files::{Files, NamedFile};
use actix_web::{dev, get, middleware, App, HttpResponse, HttpServer, Responder};

mod data;
mod error;
mod messages;
mod sm;
mod sm_actor;
mod socket;

const FRONTEND_PATH: &str = "./front/dist/";
const INDEX_PATH: &str = "./front/dist/index.html";

#[get("/test")]
async fn test() -> impl Responder {
    let p = data::Project::new("lol", "4", &["_ZZ8oyZUGn8".to_string()]);
    let res = sm::analyze(&p, "1 2").await;
    match res {
        Ok(analysis_results) => HttpResponse::Ok().json(&analysis_results),
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
