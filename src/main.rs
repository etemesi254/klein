//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

mod config;

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

}

impl AppContext {
    fn new(app_config: AppConfig) -> AppContext {
        return AppContext {
            round_robin: Arc::new(AtomicU64::new(0)),
            app_config: Arc::new(app_config),
            last_hb_time: Arc::new(AtomicU64::new(0)),
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

async fn add_server(State(ctx): State<Arc<AppContext>>, Json(payload): Json<SingleServer>) -> Json<HeartBeatResp> {
    trace!("Starting server add");
    let start = std::time::Instant::now();
    trace!("Server details:{:#?}",payload);
    match ctx.app_config.servers.write() {
        Ok(mut writer) => {
            writer.push(payload);
        }
        Err(e) => {
            error!("Could not add server, poisoned mutex, reason:{:?}",e);
        }
    }
    let stop = Instant::now();
    trace!("Took {:?} ms to add server", stop.duration_since(start).as_millis());
    return heartbeat(State(ctx)).await;
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
                .route("/home",get(home_endpoint))
                .route("/add", post(add_server))
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


#[derive(Serialize, Debug, Default)]
struct HeartBeatInfo {
    alive: bool,
    name: String,
    host: String,
    port: u16,
    status_code: Option<u16>,
    status_text: Option<String>,
    time_taken_ms: u64,
    error: Option<String>,
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

#[derive(Serialize)]
struct HeartBeatResp {
    request_time: u64,
    server_hb: Vec<HeartBeatInfo>,
}

async fn heartbeat(State(ctx): State<Arc<AppContext>>,
) -> Json<HeartBeatResp> {
    // get the current time
    let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).expect("time went backwards");
    ctx.last_hb_time.swap(now.as_secs(), Ordering::Acquire);
    let mut hb_time = vec![];

    let servers = ctx.app_config.servers.read().unwrap();
    // loop through all the configs and see if they are alive
    for server in servers.iter() {
        let req_start = Instant::now();
        // make a request
        let server_port = format!("http://{}:{}/heartbeat", server.host, server.port);

        let mut dummy_info = HeartBeatInfo::default();

        dummy_info.host = server.host.clone();
        dummy_info.port = server.port;
        match ureq::head(&server_port).call() {
            Ok(c) => {
                dummy_info.status_code = Some(c.status());
                dummy_info.status_text = Some(c.status_text().to_string());
                dummy_info.alive = true;
            }
            Err(e) => {
                dummy_info.error = Some(e.to_string());

                if let Some(resp) = e.into_response() {
                    dummy_info.status_code = Some(resp.status());
                    dummy_info.status_text = Some(resp.status_text().to_string());
                }
            }
        }
        let req_end = Instant::now();
        dummy_info.time_taken_ms = req_end.duration_since(req_start).as_millis() as u64;
        hb_time.push(dummy_info);
    }
    Json(HeartBeatResp { request_time: now.as_secs(), server_hb: hb_time })
}