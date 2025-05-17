#[macro_export]
macro_rules! impl_serde_jsonb {
    ($(#[$attr:meta])* $name:ident, $type:literal) => {
        $(#[$attr])*
        impl From<$name> for serde_json::Value {
            fn from(value: $name) -> Self {
                serde_json::to_value(&value).unwrap_or(serde_json::Value::Null)
            }
        }

        $(#[$attr])*
        impl From<serde_json::Value> for $name {
            fn from(json: serde_json::Value) -> Self {
                serde_json::from_value(json).expect(concat!(
                    "Failed to deserialize ",
                    stringify!($name),
                    " from JsonValue"
                ))
            }
        }

        $(#[$attr])*
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                sqlx::postgres::PgTypeInfo::with_name($type)
            }
        }

        $(#[$attr])*
        impl<'q> sqlx::encode::Encode<'q, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'q>,
            ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
                let json_value = match serde_json::to_value(self) {
                    Ok(value) => value,
                    Err(err) => return Err(Box::new(err)),
                };
                <serde_json::Value as sqlx::encode::Encode<sqlx::Postgres>>::encode_by_ref(
                    &json_value,
                    buf,
                )
            }
        }

        $(#[$attr])*
        impl<'r> sqlx::decode::Decode<'r, sqlx::Postgres> for $name {
            fn decode(
                value: sqlx::postgres::PgValueRef<'r>,
            ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                let json_value =
                    <serde_json::Value as sqlx::decode::Decode<sqlx::Postgres>>::decode(value)?;
                serde_json::from_value(json_value)
                    .map_err(|e| {
                        let err_msg = format!("Failed to decode {} from JSON: {}", stringify!($name), e);
                        Box::<dyn std::error::Error + Send + Sync>::from(err_msg)
                    })
            }
        }
    };
    ($(#[$attr:meta])* $name:ident) => {
        impl_serde_jsonb!($(#[$attr])* $name, "jsonb");
    };
}
