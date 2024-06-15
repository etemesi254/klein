use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Instant, UNIX_EPOCH};
use axum::extract::State;
use axum::Json;
use serde::Serialize;
use crate::AppContext;

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
pub struct HeartBeatResp {
    request_time: u64,
    server_hb: Vec<HeartBeatInfo>,
}

pub async fn heartbeat(State(ctx): State<Arc<AppContext>>,
) -> Json<HeartBeatResp> {
    // get the current time
    let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).expect("time went backwards");
    ctx.last_hb_time.swap(now.as_secs(), Ordering::Acquire);
    let mut hb_time = vec![];

    let servers = ctx.hash_server.read().unwrap().server_containers();
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