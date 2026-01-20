// Copyright 2025 Stickerbomb Maintainers
// SPDX-License-Identifier: Apache-2.0

//! Operator entrypoint

use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder, get, middleware, web::Data,
};
use kube::client;
use stickerbomb::{controller, lease::run_leader_election, telemetry};
use tokio::{pin, signal, sync::watch};
use tracing::{info, instrument};

#[get("/health")]
async fn health(_: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json("healthy")
}

#[get("/")]
async fn index(c: Data<controller::State>, _: HttpRequest) -> impl Responder {
    let d = c.diagnostics().await;
    HttpResponse::Ok().json(&d)
}

#[tokio::main]
#[instrument(level = "info", target = "operator::main", name = "main")]
async fn main() -> anyhow::Result<()> {
    telemetry::init()?;

    let state = controller::State::default();
    let client = client::Client::try_default().await?;

    let (leader_tx, leader_rx) = watch::channel(false);

    let controller = controller::run(client.clone(), state.clone(), leader_rx);
    pin!(controller);

    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(state.clone()))
            .wrap(middleware::Logger::default().exclude("/health"))
            .service(health)
            .service(index)
    })
    .bind("0.0.0.0:8080")?
    .shutdown_timeout(5);

    tokio::select! {
        () = &mut controller => info!("controller exited"),
        res = server.run() => res?,
        () = run_leader_election(client.clone(), leader_tx.clone()) =>  info!("leader election exited"),
        _ = signal::ctrl_c() => {
            let _ = leader_tx.send(false);
            info!("received shutdown signal");
        },
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::State;
    use actix_web::{App, test};

    #[actix_web::test]
    async fn test_health_endpoint() {
        let app = test::init_service(App::new().service(health)).await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body, "healthy");
    }

    #[actix_web::test]
    async fn test_index_endpoint() {
        let state = State::default();
        let app =
            test::init_service(App::new().app_data(Data::new(state.clone())).service(index)).await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        assert_eq!(resp.status(), actix_web::http::StatusCode::OK);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_object(), "Response should be a JSON object");
    }
}
