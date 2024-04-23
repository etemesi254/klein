use std::error::Error;
use std::sync::{Arc, LockResult};
use axum::extract::State;
use axum::Json;
use log::error;
use serde::Serialize;
use tracing_subscriber::field::display::Messages;
use crate::AppContext;


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
            error!("An error occured, poisoned mutex: {:?}",e);
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