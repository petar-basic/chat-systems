use axum::extract::Request;
use axum::http::header::CONTENT_TYPE;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

pub fn install_recorder() -> anyhow::Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus recorder: {e}"))?;
    Ok(handle)
}

pub async fn track_metrics(req: Request, next: Next) -> Response {
    let start = std::time::Instant::now();
    let method = req.method().clone();
    let response = next.run(req).await;
    let status = response.status().as_u16();
    let elapsed = start.elapsed().as_secs_f64();
    ::metrics::counter!(
        "http_requests_total",
        "method" => method.to_string(),
        "status" => status.to_string(),
    )
    .increment(1);
    ::metrics::histogram!("http_request_duration_seconds", "method" => method.to_string())
        .record(elapsed);
    response
}

pub fn router(handle: PrometheusHandle) -> Router {
    Router::new()
        .route("/metrics", get(render))
        .with_state(handle)
}

async fn render(
    axum::extract::State(handle): axum::extract::State<PrometheusHandle>,
) -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4")],
        handle.render(),
    )
}
