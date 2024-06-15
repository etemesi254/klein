use lazy_static::lazy_static;
use prometheus::{CounterVec, Histogram, histogram_opts, labels, opts, register_counter, register_counter_vec, register_gauge, register_histogram, register_histogram_vec};
use prometheus::{Counter, Encoder, Gauge, HistogramVec, TextEncoder};

lazy_static! {
    pub static ref HTTP_COUNTER: Counter = register_counter!(opts!(
        "klein_http_requests_total",
        "Number of HTTP requests made.",
        labels! {"handler" => "all",}
    ))
    .unwrap();
    pub static ref HTTP_NUM_REQUESTS: Gauge = register_gauge!(opts!(
        "klein_num_http_requests",
        "Number of requests in a particular time",
        labels! {"handler" => "all",}
    ))
    .unwrap();
    pub static ref HTTP_REQ_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "klein_http_request_duration_seconds",
        "The HTTP request latencies in seconds.",
        &["handler"]
    )
    .unwrap();

    pub static ref HTTP_RESPONSE_STATUS: CounterVec = register_counter_vec!(
        "klein_http_response_status_code",
        "Number of requests in a particular time",
        &["handler","status_code"]
    ).unwrap();
}
