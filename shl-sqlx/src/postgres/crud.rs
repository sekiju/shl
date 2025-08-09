use sqlx::{Executor, Postgres};

#[derive(Debug, thiserror::Error)]
pub enum CrudError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),

    #[error("no rows affected")]
    NoRows,
}

pub trait TableMeta: Sized {
    const QUAL_TABLE: &'static str;
    const COLS: &'static [&'static str];
    const PK_COLS: &'static [&'static str];
    type Id;
}

pub trait Readable: TableMeta {
    const SQL_SELECT_BY_PK: &'static str;
    const SQL_DELETE_BY_PK: &'static str;

    async fn find_by_id<'e, E>(exec: E, id: Self::Id) -> Result<Self, CrudError>
    where
        Self: Sized,
        E: Executor<'e, Database = Postgres> + Send;

    async fn delete_by_id<'e, E>(exec: E, id: Self::Id) -> Result<u64, CrudError>
    where
        E: Executor<'e, Database = Postgres> + Send;
}

pub trait Insertable: TableMeta {
    const INSERT_COLS: &'static [&'static str];
    const SQL_INSERT: &'static str;

    async fn insert<'e, E>(&self, exec: E) -> Result<u64, CrudError>
    where
        E: Executor<'e, Database = Postgres> + Send;
}

pub trait Updatable: TableMeta {
    const SQL_UPDATE: &'static str;

    async fn update<'e, E>(&self, exec: E) -> Result<u64, CrudError>
    where
        E: Executor<'e, Database = Postgres> + Send;
}
