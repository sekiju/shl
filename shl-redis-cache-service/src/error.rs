#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[cfg(feature = "rustis")]
    #[error(transparent)]
    Rustis(#[from] rustis::Error),
}
