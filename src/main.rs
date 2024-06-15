//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

mod config;
mod load_balancer;
mod consistent_hashing;
mod heartbeat;
mod prometheus_stats;

use std::io::Read;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant};
use axum::{routing::get, Router, Json};
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{StatusCode};
use axum::response::Response;
use axum::routing::{any, post};
use log::{error, info, trace, warn};
use nanorand::Rng;
use prometheus::{Encoder, TextEncoder};
use serde::{Serialize};
use tracing_subscriber::prelude::*;
use crate::config::{AppConfig, read_config, SingleServer};
use crate::consistent_hashing::{ServerPool};
use crate::heartbeat::{heartbeat};
use crate::load_balancer::{add_server, remove_server, rep};
use crate::prometheus_stats::{HTTP_COUNTER, HTTP_NUM_REQUESTS, HTTP_REQ_HISTOGRAM, HTTP_RESPONSE_STATUS};

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
    hash_server: Arc<RwLock<ServerPool>>,
    // App configuration
    app_config: Arc<AppConfig>,
    // Last time we had a heartbeat from the server
    last_hb_time: Arc<AtomicU64>,
    port: Arc<AtomicU64>,
    request_rand_gen: Arc<Mutex<nanorand::WyRand>>,
}

impl AppContext {
    fn new(app_config: AppConfig) -> AppContext {
        return AppContext {
            hash_server: Arc::new(RwLock::new(ServerPool::new(0))),
            app_config: Arc::new(app_config),
            last_hb_time: Arc::new(AtomicU64::new(0)),
            port: Arc::new(AtomicU64::new(18000)),
            request_rand_gen: Arc::new(Mutex::new(nanorand::WyRand::new_seed(37))),
        };
    }
}

fn handle_request(mut req: ureq::Request, server_name: &str, incoming: axum::extract::Request) -> Response {
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
            HTTP_RESPONSE_STATUS.with_label_values(&[status.to_string().as_str(), server_name]).inc();

            let end = Instant::now();
            trace!("Took {:?} ms to get response\n",end.duration_since(start).as_millis());
            // return response
            Response::builder().status(status).body(Body::from(data)).unwrap()
        }
        Err(f) => {
            warn!("Error occurred when making request:  {:?}",f);
            if let Some(resp) = f.into_response() {
                HTTP_RESPONSE_STATUS.with_label_values(&[resp.status().to_string().as_str(), server_name]).inc();

                return Response::builder().status(resp.status()).body(Body::from(resp.into_string().unwrap())).unwrap();
            }
            Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::from("An Error occurred, please fix it")).unwrap()
        }
    };
}


fn get_server(values: &AppContext, to: String) -> Option<SingleServer> {
    // get the request generator indicator
    let request_rand_gen = values.request_rand_gen.lock().unwrap().generate_range(100_000..999_999);

    info!("Assigning request {} id {}",to,request_rand_gen);

    match values.hash_server.write().unwrap().get_server_container(request_rand_gen) {
        None => {
            error!("Could not get the server");
            None
        }
        Some(server) => {
            info!("Using server {} (id={}) for request {}", server.name,server.id,to);
            Some(server)
        }
    }
}


async fn re_router(State(ctx): State<Arc<AppContext>>, req: Request) -> Response {
    HTTP_COUNTER.inc();

    // choose server
    match get_server(&ctx, req.uri().to_string()) {
        Some(server) => {
            let timer = HTTP_REQ_HISTOGRAM.with_label_values(&[server.name.as_str()]).start_timer();

            HTTP_NUM_REQUESTS.inc();
            let uri = req.uri();
            // create base url
            let base_url = format!("http://{}:{}{}", server.host, server.port, uri.path_and_query().map(|c| c.to_string()).unwrap_or(String::new()));
            trace!("URL {}",base_url);
            let method = req.method().to_owned();
            let req_method = ureq::request(method.to_string().as_str(), &base_url);

            let c = handle_request(req_method, &server.name, req);

            timer.observe_duration();

            HTTP_NUM_REQUESTS.dec();
            return c;
        }
        None => {
            let response = Response::new(Body::from("no backend server is up"));
            let (mut parts, body) = response.into_parts();

            parts.status = StatusCode::INTERNAL_SERVER_ERROR;
            let response = Response::from_parts(parts, body);
            return response;
        }
    };
}

async fn stats() -> Response<Body> {
    let encoder = TextEncoder::new();

    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer).unwrap();

    let response = Response::builder()
        .status(200)
        .header(axum::http::header::CONTENT_TYPE, encoder.format_type())
        .body(Body::from(buffer))
        .unwrap();

    response
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
                .route("/metrics", get(stats))
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
    Json(match get_server(&ctx, "/home".to_string()) {
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

