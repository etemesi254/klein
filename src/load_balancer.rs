use std::fmt::Pointer;
use std::process::Command;
use std::sync::{Arc, LockResult};
use std::sync::atomic::Ordering;
use std::time::Instant;
use axum::extract::State;
use axum::Json;
use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use tracing_subscriber::field::display::Messages;
use tracing_subscriber::fmt::format;
use crate::AppContext;
use crate::config::SingleServer;
use crate::heartbeat::{heartbeat, HeartBeatResp};


#[derive(Serialize)]
#[allow(non_snake_case)]
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

pub async fn add_server(State(ctx): State<Arc<AppContext>>, Json(payload): Json<RequestLayout>) -> Json<Vec<RmResponse>> {
    trace!("Starting server add");
    let start = std::time::Instant::now();
    let mut de = vec![];
    match ctx.hash_server.write() {
        Ok(mut writer) => {
            for name in &payload.hostnames {
                let new_port = ctx.port.fetch_add(1, Ordering::AcqRel);

                let command = Command::new("docker")
                    .arg("run")
                    .arg("-d")
                    .arg("--name")
                    .arg(name)
                    .arg("-p")
                    .arg(format!("{}:8000", new_port))
                    .arg("-e")
                    .arg(format!("SERVER_ID={}", name))
                    .arg("nasa_api").output();

                match command {
                    Ok(e) => {
                        if e.status.success() {
                            writer.add_server(name.to_string(), "127.0.0.1".to_string(), new_port as u16);
                            info!("Successfully added server: Output: {:?}",e);
                            de.push(RmResponse {
                                name: name.to_owned(),
                                status: e.status.code().unwrap_or(-255),
                                stdout: String::from_utf8_lossy(&e.stdout).trim().to_string(),
                                stderr: String::from_utf8_lossy(&e.stderr).trim().to_string(),
                            });
                        } else {
                            error!("Could not add a server  status code failed");
                            de.push(RmResponse {
                                name: name.to_owned(),
                                status: e.status.code().unwrap_or(-255),
                                stdout: String::from_utf8_lossy(&e.stdout).trim().to_string(),
                                stderr: String::from_utf8_lossy(&e.stderr).trim().to_string(),
                            });
                        }
                    }
                    Err(e) => {
                        error!("An error occurred :{}",e);
                    }
                }
            }
        }
        Err(e) => {
            error!("Could not add server, poisoned mutex, reason:{:?}",e);
        }
    }
    let stop = Instant::now();
    trace!("Took {:?} ms to add server", stop.duration_since(start).as_millis());
    return Json(de);
}

fn create_docker_instance() {}

/// `rm` command endpoint
#[derive(Deserialize)]
pub struct RequestLayout {
    n: usize,
    hostnames: Vec<String>,
}

#[derive(Serialize)]
pub struct RmResponse {
    name: String,
    status: i32,
    stdout: String,
    stderr: String,
}

///  Endpoint (/rm, method=DELETE): This endpoint removes server instances in the load balancer to scale down with
/// decreasing client or system maintenance. The endpoint expects a JSON payload that mentions the number of instances
/// to be removed and their preferred hostnames (same as container name in docker) in a list. An example request and response
/// is below.
pub async fn remove_server(State(ctx): State<Arc<AppContext>>, Json(payload): Json<RequestLayout>) -> Json<Vec<RmResponse>> {

    //if
    //docker rm -f mycontainer
    let mut de = vec![];

    match ctx.app_config.servers.write() {
        Ok(mut writer) => {
            let new_port = ctx.port.fetch_add(1, Ordering::AcqRel);

            for name in &payload.hostnames {
                let command = Command::new("docker")
                    .arg("rm")
                    .arg("-f")
                    .arg(name)
                    .output();
                match command {
                    Ok(e) => {
                        info!("Successfully removed server: Output: {:?}",e);
                        de.push(RmResponse {
                            name: name.to_owned(),
                            status: e.status.code().unwrap_or(-255),
                            stdout: String::from_utf8_lossy(&e.stdout).trim().to_string(),
                            stderr: String::from_utf8_lossy(&e.stderr).trim().to_string(),
                        });
                    }
                    Err(e) => {
                        error!("An error occurred :{}",e);
                    }
                }
                writer.iter().position(|c| &c.name == name);
            }
        }
        Err(e) => {
            error!("Could not add server, poisoned mutex, reason:{:?}",e);
        }
    }
    Json(de)
}