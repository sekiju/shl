use chrono::{DateTime, Utc};
use shl_sqlx::uuid::uuidv7_and_created_at;
use sqlx::FromRow;
use uuid::Uuid;
use shl_sqlx::{Table, Insertable, Updatable};
use shl_sqlx::postgres::{Readable, Insertable, Updatable};

#[derive(Debug, FromRow, Table, Insertable, Updatable)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub permissions_bitmask: i64,
    pub avatar_s3_key: Option<String>,
    pub email_verified: bool,
    pub banned: bool,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, PartialEq, PartialOrd, sqlx::Type)]
#[sqlx(type_name = "integration_kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IntegrationKind {
    Google,
}

#[derive(Debug, FromRow, Table, Insertable)]
#[table(pk("kind", "external_identifier"))]
pub struct Integration {
    pub kind: IntegrationKind,
    pub external_identifier: String,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect_lazy(&std::env::var("DATABASE_URL")?)?;

    let (id, created_at) = uuidv7_and_created_at();
    let mut user = User {
        id,
        name: "alice".into(),
        email: None,
        password_hash: None,
        permissions_bitmask: 0,
        avatar_s3_key: None,
        email_verified: false,
        banned: false,
        last_seen_at: created_at,
        created_at,
        updated_at: None,
    };
    user.insert(&pool).await?;

    user.avatar_s3_key = Some("file.png".to_owned());
    user.update(&pool).await?;

    let integration = Integration {
        kind: IntegrationKind::Google,
        external_identifier: "123456789".to_owned(),
        user_id: id,
        created_at,
    };
    integration.insert(&pool).await?;

    let _ = Integration::find_by_id(&pool, (IntegrationKind::Google, "123456789".to_owned())).await?;

    Ok(())
}
