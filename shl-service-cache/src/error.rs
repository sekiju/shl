#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Redis(#[from] rustis::Error),
}
