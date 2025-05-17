#[macro_export]
macro_rules! impl_string_varchar {
    ($name:ident) => {
        impl sqlx::Type<sqlx::Postgres> for $name {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                sqlx::postgres::PgTypeInfo::with_name("varchar")
            }
        }

        impl<'q> sqlx::encode::Encode<'q, sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut <sqlx::Postgres as sqlx::Database>::ArgumentBuffer<'q>,
            ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
                let string_value = self.to_string();
                <String as sqlx::encode::Encode<sqlx::Postgres>>::encode_by_ref(&string_value, buf)
            }
        }

        impl<'r> sqlx::decode::Decode<'r, sqlx::Postgres> for $name {
            fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                let string_value = <String as sqlx::decode::Decode<sqlx::Postgres>>::decode(value)?;
                string_value
                    .parse::<$name>()
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
        }
    };
}