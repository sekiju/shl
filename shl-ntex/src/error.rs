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
