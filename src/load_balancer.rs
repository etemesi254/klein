use std::error::Error;
use std::sync::{Arc, LockResult};
use std::time::Instant;
use axum::extract::State;
use axum::Json;
use log::{error, trace};
use serde::{Deserialize, Serialize};
use tracing_subscriber::field::display::Messages;
use crate::AppContext;
use crate::config::SingleServer;
use crate::heartbeat::{heartbeat, HeartBeatResp};


#[derive(Serialize)]
pub struct RepResponseMessage {
    N: usize,
    replicas: Vec<String>,
}

#[derive(Serialize)]
pub struct RespResponse {
    message: RepResponseMessage,
    status: String,
}

/// The rep endpoint
///
/// Endpoint (/rep, method=GET): This endpoint only returns the status of the replicas managed by the load balancer.
/// The response contains the number of replicas and their hostname in the docker internal network:n1 as mentioned in
/// Fig. 1. An example response is shown below.
pub async fn rep(State(ctx): State<Arc<AppContext>>) -> Json<RespResponse> {
    Json(match ctx.app_config.servers.read() {
        Ok(c) => {
            RespResponse {
                message: RepResponseMessage {
                    N: c.len(),
                    replicas: c.iter().map(|c| c.name.to_string()).collect(),
                },
                status: "successful".to_string(),
            }
        }
        Err(e) => {
            error!("An error occurred, poisoned mutex: {:?}",e);
            RespResponse {
                message: RepResponseMessage {
                    N: 0,
                    replicas: vec![],
                },
                status: "error".to_string(),
            }
        }
    })
}

pub async fn add_server(State(ctx): State<Arc<AppContext>>, Json(payload): Json<SingleServer>) -> Json<HeartBeatResp> {
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


/// `rm` command endpoint
#[derive(Deserialize)]
struct RmRequestLayout {
    n: usize,
    hostnames: Vec<String>,
}

///  Endpoint (/rm, method=DELETE): This endpoint removes server instances in the load balancer to scale down with
/// decreasing client or system maintenance. The endpoint expects a JSON payload that mentions the number of instances
/// to be removed and their preferred hostnames (same as container name in docker) in a list. An example request and response
/// is below.
pub async  fn remove_server(State(ctx): State<Arc<AppContext>>, Json(payload): Json<RmRequestLayout>){

    //if

}