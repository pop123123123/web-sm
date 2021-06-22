use actix_files::{Files, NamedFile};
use actix_web::{dev, middleware, App, HttpServer};

mod data;
mod sm;

const FRONTEND_PATH: &str = "./front/dist/";
const INDEX_PATH: &str = "./front/dist/index.html";

fn test() {
    let p = data::Project {
        seed: "4".to_string(),
        video_urls: vec!["_ZZ8oyZUGn8".to_string()],
        name: "lol".to_string(),
    };
    let res = sm::analyze(&p, "est");
    dbg!(res.unwrap());
}

/// Run actix web server
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    test();
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3333".to_string())
        .parse()
        .expect("PORT must be a number");

    HttpServer::new(move || {
        App::new().wrap(middleware::Compress::default()).service(
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
