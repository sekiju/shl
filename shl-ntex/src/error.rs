pub use ntex_error_macro::*;
use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct NtexErrorResponse {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<HashMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use crate as shl_ntex;
    use crate::error::NtexError;
    use ntex::http::StatusCode;

    #[derive(thiserror::Error, Debug, NtexError)]
    pub enum SubServiceError {
        #[error("Unauthorized")]
        #[ntex_response(status=StatusCode::UNAUTHORIZED)]
        Unauthorized,

        #[error(transparent)]
        Std(#[from] std::io::Error),
    }

    #[derive(thiserror::Error, Debug, NtexError)]
    pub enum AuthorizationError {
        #[error("Unauthorized")]
        #[ntex_response(status=StatusCode::UNAUTHORIZED)]
        Unauthorized,

        #[error("Incorrect password")]
        #[ntex_response(status=StatusCode::BAD_REQUEST, name="bad_password")]
        PasswordIncorrect,

        #[error("Invalid grant type")]
        InvalidGrantType,

        #[error(transparent)]
        #[ntex_response(transparent)]
        SubService(#[from] SubServiceError),

        #[error("IO wrapper")]
        StdWrapper(std::io::Error),
    }
}
