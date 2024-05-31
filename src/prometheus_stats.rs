use lazy_static::lazy_static;
use prometheus::{labels, opts, register_counter, register_gauge, register_histogram_vec};
use prometheus::{Counter, Encoder, Gauge, HistogramVec, TextEncoder};

lazy_static! {
    pub static ref HTTP_COUNTER: Counter = register_counter!(opts!(
        "klein_http_requests_total",
        "Number of HTTP requests made.",
        labels! {"handler" => "all",}
    ))
    .unwrap();
    pub static ref HTTP_BODY_GAUGE: Gauge = register_gauge!(opts!(
        "klein",
        "The HTTP response sizes in bytes.",
        labels! {"handler" => "all",}
    ))
    .unwrap();
    pub static ref HTTP_REQ_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "klein_http_request_duration_seconds",
        "The HTTP request latencies in seconds.",
        &["handler"]
    )
    .unwrap();
}
