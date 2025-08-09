use sqlx::{Error, Executor, Postgres};

pub trait TableMeta: Sized {
    const QUAL_TABLE: &'static str;
    const COLS: &'static [&'static str];
    const PK_COLS: &'static [&'static str];
    type Id;
}

pub trait Readable: TableMeta {
    const SQL_SELECT_BY_PK: &'static str;
    const SQL_DELETE_BY_PK: &'static str;

    fn find_by_id<'e, E>(exec: E, id: Self::Id) -> impl Future<Output = Result<Self, Error>> + Send + 'e
    where
        Self: Sized + 'e,
        E: Executor<'e, Database = Postgres> + Send + 'e;

    fn delete_by_id<'e, E>(exec: E, id: Self::Id) -> impl Future<Output = Result<u64, Error>> + Send + 'e
    where
        E: Executor<'e, Database = Postgres> + Send + 'e;
}

pub trait Insertable: TableMeta {
    const INSERT_COLS: &'static [&'static str];
    const SQL_INSERT: &'static str;

    fn insert<'e, E>(&'e self, exec: E) -> impl Future<Output = Result<u64, Error>> + Send + 'e
    where
        E: Executor<'e, Database = Postgres> + Send + 'e;
}

pub trait Updatable: TableMeta {
    const SQL_UPDATE: &'static str;

    fn update<'e, E>(&'e self, exec: E) -> impl Future<Output = Result<u64, Error>> + Send + 'e
    where
        E: Executor<'e, Database = Postgres> + Send + 'e;
}
