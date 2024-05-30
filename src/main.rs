//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

mod config;
mod load_balancer;
mod consistent_hashing;
mod heartbeat;

use std::io::Read;
use std::sync::{Arc};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, UNIX_EPOCH};
use axum::{routing::get, Router, Json};
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{StatusCode};
use axum::response::Response;
use axum::routing::{any, post};
use log::{error, info, trace, warn};
use serde::{Deserialize, Serialize};
use tracing_subscriber::prelude::*;
use crate::config::{AppConfig, read_config, SingleServer};
use crate::heartbeat::{heartbeat, HeartBeatResp};
use crate::load_balancer::{add_server, remove_server, rep};

/// Initialize the logging library
///
/// We use the tracing library to do logging
/// setting it to be trace for the klein binary
fn init_log() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "klein=trace".into()), // add ",tower_http=debug"
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    info!("Initialized logging")
}

#[derive(Clone)]
struct AppContext {
    round_robin: Arc<AtomicU64>,
    // App configuration
    app_config: Arc<AppConfig>,
    // Last time we had a heartbeat from the server
    last_hb_time: Arc<AtomicU64>,
    port: Arc<AtomicU64>,
}

impl AppContext {
    fn new(app_config: AppConfig) -> AppContext {
        return AppContext {
            round_robin: Arc::new(AtomicU64::new(0)),
            app_config: Arc::new(app_config),
            last_hb_time: Arc::new(AtomicU64::new(0)),
            port: Arc::new(AtomicU64::new(18000)),
        };
    }
}

fn handle_request(mut req: ureq::Request, incoming: axum::extract::Request) -> Response {
    // add headers from request
    for (k, v) in incoming.headers() {
        req = req.set(&k.to_string(), v.to_str().unwrap());
    }

    let start = Instant::now();

    // call it finally
    return match req.call() {
        Ok(e) => {
            let mut data = Vec::new();
            let status = e.status();

            e.into_reader().read_to_end(&mut data).unwrap();
            let end = Instant::now();
            trace!("Took {:?} ms to get response\n",end.duration_since(start).as_millis());
            // return response
            Response::builder().status(status).body(Body::from(data)).unwrap()
        }
        Err(f) => {
            warn!("Error occurred when making request:  {:?}",f);
            if let Some(resp) = f.into_response() {
                return Response::builder().status(resp.status()).body(Body::from(resp.into_string().unwrap())).unwrap();
            }
            Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::from("An Error occurred, please fix it")).unwrap()
        }
    };
}


fn get_server(values: &AppContext) -> Option<SingleServer> {
    let current_val = values.round_robin.fetch_add(1, Ordering::Acquire);
    if let Ok(v) = values.app_config.servers.read() {
        let pos = current_val as usize % v.len();
        let server = v[pos].clone();
        trace!("Using server {:?} for the request", server.name);
        return Some(server);
    }
    return None;
}

async fn re_router(State(ctx): State<Arc<AppContext>>, req: Request) -> Response {
    // choose server
    let server = get_server(&ctx).unwrap();
    let uri = req.uri();
    // create base url
    let base_url = format!("http://{}:{}{}", server.host, server.port, uri.path_and_query().map(|c| c.to_string()).unwrap_or(String::new()));
    trace!("URL {}",base_url);
    let method = req.method().to_owned();
    let req_method = ureq::request(method.to_string().as_str(), &base_url);

    let c = handle_request(req_method, req);
    return c;
}

#[tokio::main]
async fn main() {
    // initialize logging
    init_log();
    // read toml file containing configs
    match read_config() {
        Ok(config) => {
            let (h, p) = (config.host.to_owned(), config.port);
            let ctx = AppContext::new(config);

            // build our application with a route
            let app = Router::new()
                .fallback(any(re_router))
                .route("/heartbeat", get(heartbeat))
                .route("/home", get(home_endpoint))
                .route("/add", post(add_server))
                .route("/rm", post(remove_server))
                .route("/rep", get(rep))
                .with_state(Arc::new(ctx));
            // run it
            match tokio::net::TcpListener::bind(format!("{}:{}", h, p))
                .await {
                Ok(listener) => {
                    info!("listening on {}\n", listener.local_addr().unwrap());
                    axum::serve(listener, app).await.unwrap();
                }
                Err(e) => {
                    error!("Could not bind to address: {e}");
                }
            }
        }
        Err(err) => {
            error!("{}",err);
        }
    }
}


#[derive(Serialize)]
struct HomeResp {
    message: String,
    status: String,
}

async fn home_endpoint(State(ctx): State<Arc<AppContext>>) -> Json<HomeResp> {
    Json(match get_server(&ctx) {
        None => {
            HomeResp {
                message: "Could not get server".to_string(),
                status: "error".to_string(),
            }
        }
        Some(chosen_server) => {
            trace!("Handling request '/home' endpoint via {}\n",chosen_server.name);
            HomeResp {
                message: format!("Hello from Server: {}", chosen_server.name),
                status: "successful".to_string(),
            }
        }
    })
}

