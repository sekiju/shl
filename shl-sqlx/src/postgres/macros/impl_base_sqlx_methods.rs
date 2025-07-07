/// # impl_base_sqlx_methods
///
/// This macro generates common database methods for your struct:
/// - `find_by_id` - fetch a record by UUID
/// - `list_all` - fetch all records ordered by created_at
/// - `insert` - insert a new record
/// - `delete` - delete a record by ID
///
/// ## Basic Usage
///
/// ```
/// impl_base_sqlx_methods!(User, "users",
///     id => Uuid,
///     username => String,
///     email => String,
///     created_at => DateTime<Utc>
/// );
/// ```
///
/// ## With Update Support
///
/// Add `with_update` to generate an update method:
///
/// ```
/// impl_base_sqlx_methods!(User, "users", with_update,
///     id => Uuid,
///     username => String,
///     email => String,
///     created_at => DateTime<Utc>,
///     updated_at => Option<DateTime<Utc>>
/// );
/// ```
///
/// ## Converting From Struct Fields
///
/// Convert your struct fields from standard Rust format:
///
/// ```
/// pub struct User {
///     pub id: Uuid,
///     pub username: String,
///     pub email: String,
///     pub created_at: DateTime<Utc>,
///     pub updated_at: Option<DateTime<Utc>>
/// }
/// ```
///
/// To macro format using regex:
/// - Find: `pub\s+(\w+):\s+([^,]+),`
/// - Replace: `$1 => $2,`
///
/// ## Dependencies
///
/// Requires:
/// - `sqlx` with PostgreSQL feature
/// - `uuid` for Uuid type
/// - `chrono` for DateTime
/// - Custom `Error` type with `RowNotFound` variant
#[macro_export]
macro_rules! impl_base_sqlx_methods {
    ($struct_name:ident, $table_name:expr, $($field_name:ident => $field_type:ty),* $(,)?) => {
        impl $struct_name {
            const FIELD_COUNT: usize = {
                let mut count = 0;
                $(
                    let _ = stringify!($field_name);
                    count += 1;
                )*
                count
            };

            pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Self, Error> {
                sqlx::query_as::<_, Self>(
                    &format!("SELECT * FROM {} WHERE id = $1", $table_name)
                )
                .bind(id)
                .fetch_one(pool)
                .await
            }

            pub async fn list_all(pool: &PgPool) -> Result<Vec<Self>, Error> {
                sqlx::query_as::<_, Self>(
                    &format!("SELECT * FROM {} ORDER BY created_at DESC", $table_name)
                )
                .fetch_all(pool)
                .await
            }

            pub async fn insert(&self, pool: &PgPool) -> Result<(), Error> {
                let field_count = Self::FIELD_COUNT;

                let query = format!(
                    "INSERT INTO {} ({}) VALUES ({})",
                    $table_name,
                    stringify!($($field_name),*),
                    (1..=field_count)
                        .map(|i| format!("${}", i))
                        .collect::<Vec<_>>()
                        .join(", ")
                );

                let mut q = sqlx::query(&query);
                $(
                    q = q.bind(&self.$field_name);
                )*

                q.execute(pool).await?;
                Ok(())
            }

            pub async fn delete(&self, pool: &PgPool) -> Result<(), Error> {
                let result = sqlx::query(
                    &format!("DELETE FROM {} WHERE id = $1", $table_name)
                )
                .bind(self.id)
                .execute(pool)
                .await?;

                if result.rows_affected() == 0 {
                    Err(Error::RowNotFound)
                } else {
                    Ok(())
                }
            }
        }
    };

    ($struct_name:ident, $table_name:expr, with_update, $($field_name:ident => $field_type:ty),* $(,)?) => {
        impl_base_sqlx_methods!($struct_name, $table_name, $($field_name => $field_type),*);

        impl $struct_name {
            pub async fn update(&self, pool: &PgPool) -> Result<(), Error> {
                let field_count = Self::FIELD_COUNT;

                let mut updates = Vec::new();
                let mut param_count = 1;
                $(
                    updates.push(format!("{} = ${}", stringify!($field_name), param_count));
                    param_count += 1;
                )*

                updates.push("updated_at = now()".to_string());

                let updates_str = updates.join(", ");

                let query = format!(
                    "UPDATE {} SET {} WHERE id = ${}",
                    $table_name,
                    updates_str,
                    field_count + 1
                );

                let mut q = sqlx::query(&query);
                $(
                    q = q.bind(&self.$field_name);
                )*
                q = q.bind(self.id);

                let result = q.execute(pool).await?;

                if result.rows_affected() == 0 {
                    Err(Error::RowNotFound)
                } else {
                    Ok(())
                }
            }
        }
    };
}
