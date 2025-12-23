//! Operator entrypoint

use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder, get, middleware, web::Data,
};
use stickerbomb::{State, run, telemetry};
use tracing::instrument;

#[get("/health")]
async fn health(_: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json("healthy")
}

#[get("/")]
async fn index(c: Data<State>, _: HttpRequest) -> impl Responder {
    let d = c.diagnostics().await;
    HttpResponse::Ok().json(&d)
}

#[tokio::main]
#[instrument(level = "info", target = "operator::main", name = "main")]
async fn main() -> anyhow::Result<()> {
    telemetry::init()?;

    let state = State::default();
    let controller = run(state.clone());

    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(state.clone()))
            .wrap(middleware::Logger::default().exclude("/health"))
            .service(health)
            .service(index)
    })
    .bind("0.0.0.0:8080")?
    .shutdown_timeout(5);

    tokio::join!(controller, server.run()).1?;
    Ok(())
}
