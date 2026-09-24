use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

pub fn init() {
    if HANDLE.get().is_some() {
        return;
    }
    match PrometheusBuilder::new().install_recorder() {
        Ok(handle) => {
            let _ = HANDLE.set(handle);
        }
        Err(err) => {
            tracing::warn!(error = %err, "prometheus recorder already installed");
        }
    }
}

pub fn render() -> String {
    HANDLE
        .get()
        .map(|handle| handle.render())
        .unwrap_or_default()
}
