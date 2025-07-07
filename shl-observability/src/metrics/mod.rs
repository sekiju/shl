cfg_if! {
    if #[cfg(feature = "ntex")] {
        mod ntex;
        pub use ntex::*;
    }
}
use cfg_if::cfg_if;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};
use std::error::Error;
use std::net::SocketAddr;
use tracing::info;

pub fn init() -> Result<(), Box<dyn Error>> {
    const EXPONENTIAL_SECONDS: &[f64] = &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];

    let listener = SocketAddr::from(([0, 0, 0, 0], 9000));

    PrometheusBuilder::new()
        .set_buckets_for_metric(Matcher::Full("http_requests_duration_seconds".to_string()), EXPONENTIAL_SECONDS)?
        .with_http_listener(listener)
        .install()?;

    info!("Prometheus metrics exporter is running on http://{}", listener);

    Ok(())
}
