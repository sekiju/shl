use std::error::Error;

#[cfg(feature = "metrics")]
pub mod metrics;
#[cfg(feature = "tracing")]
pub mod tracing;

/// Initialize observability features such as tracing and metrics based on enabled features.
/// Returns an error if any initialization (e.g., metrics) fails.
pub fn init() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "tracing")]
    tracing::init();
    #[cfg(feature = "metrics")]
    metrics::init()?;

    Ok(())
}
