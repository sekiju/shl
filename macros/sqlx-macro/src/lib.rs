use proc_macro::TokenStream;
use proc_macro_error::{abort, proc_macro_error};
use quote::quote;
use syn::spanned::Spanned;
use syn::{
    Attribute, Data, DeriveInput, Fields, Ident, Lit, LitStr, Meta, MetaNameValue, Result as SynResult, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

#[derive(Default, Clone)]
struct ModelCfg {
    schema: String,
    table: String,
    pk_cols: Vec<String>,
    insert_skip: Vec<String>,
    skip_update: Vec<String>,
}

impl ModelCfg {
    fn apply_default(ty_ident: &Ident) -> Self {
        Self {
            schema: "public".into(),
            table: to_snake_plural(&ty_ident.to_string()),
            pk_cols: vec!["id".into()],
            insert_skip: vec![],
            skip_update: vec!["id".into(), "created_at".into()],
        }
    }
}

enum TableArg {
    Schema(LitStr),
    Table(LitStr),
    PkList(Punctuated<LitStr, Comma>),
    InsertSkip(Punctuated<LitStr, Comma>),
    SkipUpdate(Punctuated<LitStr, Comma>),
}

impl Parse for TableArg {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let key: Ident = input.parse()?;
        if key == "schema" || key == "table" || key == "pk" {
            if input.peek(Token![=]) {
                input.parse::<Token![=]>()?;
                let val: LitStr = input.parse()?;
                return Ok(match key.to_string().as_str() {
                    "schema" => TableArg::Schema(val),
                    "table" => TableArg::Table(val),
                    _ => unreachable!(),
                });
            }
        }
        if key == "pk" {
            let content;
            syn::parenthesized!(content in input);
            let list = Punctuated::parse_terminated(&content)?;
            return Ok(TableArg::PkList(list));
        }
        if key == "insert_skip" {
            let content;
            syn::parenthesized!(content in input);
            let list = Punctuated::parse_terminated(&content)?;
            return Ok(TableArg::InsertSkip(list));
        }
        if key == "skip_update" {
            let content;
            syn::parenthesized!(content in input);
            let list = Punctuated::parse_terminated(&content)?;
            return Ok(TableArg::SkipUpdate(list));
        }

        Err(syn::Error::new(
            key.span(),
            "Unknown key in #[crud(..)]. Expected: schema=..., table=..., pk(...)/pk=\"...\", insert_skip(...), skip_update(...).",
        ))
    }
}

fn to_snake_plural(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
        } else {
            out.push(ch);
        }
    }
    if !out.ends_with('s') {
        out.push('s');
    }
    out
}

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

struct TableArgs {
    items: Punctuated<TableArg, Token![,]>,
}

impl Parse for TableArgs {
    fn parse(input: ParseStream) -> SynResult<Self> {
        Ok(Self {
            items: input.parse_terminated(TableArg::parse, Comma)?,
        })
    }
}

fn parse_model_cfg(attrs: &[Attribute], ty_ident: &Ident) -> SynResult<ModelCfg> {
    let mut cfg = ModelCfg::apply_default(ty_ident);

    for attr in attrs {
        if !attr.path().is_ident("table") {
            continue;
        }

        let args: TableArgs = attr.parse_args()?;
        for item in args.items {
            match item {
                TableArg::Schema(s) => cfg.schema = s.value(),
                TableArg::Table(s) => cfg.table = s.value(),
                TableArg::PkList(list) => {
                    let span = list.span();
                    cfg.pk_cols = list.into_iter().map(|x| x.value()).collect();
                    if cfg.pk_cols.is_empty() {
                        return Err(syn::Error::new(span, "pk(...) cannot be empty"));
                    }
                }
                TableArg::InsertSkip(list) => {
                    cfg.insert_skip = list.into_iter().map(|x| x.value()).collect();
                }
                TableArg::SkipUpdate(list) => {
                    cfg.skip_update = list.into_iter().map(|x| x.value()).collect();
                }
            }
        }
    }
    Ok(cfg)
}

fn field_rename(attrs: &Vec<Attribute>, fallback: &str) -> String {
    for a in attrs {
        if !a.path().is_ident("table") {
            continue;
        }
        let parsed = a.parse_args_with(|input: ParseStream| -> syn::Result<Option<String>> {
            if input.is_empty() {
                return Ok(None);
            }
            let key: syn::Ident = input.parse()?;
            if key == "rename" {
                input.parse::<Token![=]>()?;
                let v: LitStr = input.parse()?;
                Ok(Some(v.value()))
            } else {
                Ok(None)
            }
        });
        if let Ok(Some(name)) = parsed {
            return name;
        }
    }
    fallback.to_string()
}

fn pk_ty_tokens(pk_types: &Vec<syn::Type>) -> proc_macro2::TokenStream {
    match pk_types.len() {
        1 => {
            let a = &pk_types[0];
            quote! { #a }
        }
        2 => {
            let a = &pk_types[0];
            let b = &pk_types[1];
            quote! { (#a, #b) }
        }
        _ => abort!(proc_macro2::Span::call_site(), "only 1-2 PK columns are supported"),
    }
}

fn where_pk(pk_cols_sql: &Vec<String>) -> String {
    match pk_cols_sql.len() {
        1 => format!("{} = $1", pk_cols_sql[0]),
        2 => format!("{} = $1 AND {} = $2", pk_cols_sql[0], pk_cols_sql[1]),
        _ => unreachable!(),
    }
}

fn placeholders(n: usize) -> String {
    (1..=n).map(|i| format!("${}", i)).collect::<Vec<_>>().join(", ")
}

struct ColInfo {
    rs_ident: Ident,
    sql_quoted: String,
    ty: syn::Type,
}

fn collect(input: &DeriveInput, cfg: &ModelCfg) -> (Vec<ColInfo>, Vec<Ident>, Vec<String>, Vec<Ident>, Vec<syn::Type>) {
    let ds = match &input.data {
        Data::Struct(ds) => ds,
        _ => abort!(input.span(), "only structs are supported"),
    };
    let named = match &ds.fields {
        Fields::Named(n) => &n.named,
        _ => abort!(ds.struct_token.span, "named fields are required"),
    };

    let mut cols = Vec::<ColInfo>::new();
    for f in named.iter() {
        let name = f.ident.clone().unwrap();
        let col = field_rename(&f.attrs, &name.to_string());
        cols.push(ColInfo {
            rs_ident: name,
            sql_quoted: format!("\"{}\"", col),
            ty: f.ty.clone(),
        });
    }

    let mut pk_idents = Vec::<Ident>::new();
    let mut pk_types = Vec::<syn::Type>::new();
    for pk in &cfg.pk_cols {
        let mut found = None;
        for f in named.iter() {
            let nm = f.ident.as_ref().unwrap();
            let col_name = field_rename(&f.attrs, &nm.to_string());
            if &nm.to_string() == pk || &col_name == pk {
                found = Some((nm.clone(), f.ty.clone()));
                break;
            }
        }
        if let Some((id, ty)) = found {
            pk_idents.push(id);
            pk_types.push(ty);
        } else {
            abort!(input.span(), format!("pk field '{}' not found", pk));
        }
    }

    let rs_names = cols.iter().map(|c| c.rs_ident.clone()).collect::<Vec<_>>();
    let cols_sql = cols.iter().map(|c| c.sql_quoted.clone()).collect::<Vec<_>>();

    (cols, rs_names, cols_sql, pk_idents, pk_types)
}

#[proc_macro_derive(Table, attributes(table))]
pub fn derive_table(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let ident = input.ident.clone();

    let cfg = match parse_model_cfg(&input.attrs, &ident) {
        Ok(c) => c,
        Err(e) => return e.into_compile_error().into(),
    };

    let (_cols, _rs_names, cols_sql, pk_idents, pk_types) = collect(&input, &cfg);
    let qual_table = format!("\"{}\".\"{}\"", cfg.schema, cfg.table);

    let cols_arr = cols_sql.iter().map(|c| syn::LitStr::new(c, input.span()));
    let pk_cols_sql = cfg.pk_cols.iter().map(|c| format!("\"{}\"", c)).collect::<Vec<_>>();
    let pk_arr = pk_cols_sql.iter().map(|c| syn::LitStr::new(c, input.span()));

    let id_ty = pk_ty_tokens(&pk_types);

    let select_sql = format!("SELECT {} FROM {} WHERE {}", cols_sql.join(", "), qual_table, where_pk(&pk_cols_sql));
    let delete_sql = format!("DELETE FROM {} WHERE {}", qual_table, where_pk(&pk_cols_sql));
    let select_lit = syn::LitStr::new(&select_sql, input.span());
    let delete_lit = syn::LitStr::new(&delete_sql, input.span());

    let bind_select = if pk_idents.len() == 1 {
        quote! { let mut q = sqlx::query_as::<_, Self>(Self::SQL_SELECT_BY_PK); q = q.bind(id); }
    } else {
        quote! { let (a,b) = id; let mut q = sqlx::query_as::<_, Self>(Self::SQL_SELECT_BY_PK); q = q.bind(a); q = q.bind(b); }
    };
    let bind_delete = if pk_idents.len() == 1 {
        quote! { let mut q = sqlx::query(Self::SQL_DELETE_BY_PK); q = q.bind(id); }
    } else {
        quote! { let (a,b) = id; let mut q = sqlx::query(Self::SQL_DELETE_BY_PK); q = q.bind(a); q = q.bind(b); }
    };

    let expanded = quote! {
        impl shl_sqlx::postgres::TableMeta for #ident {
            type Id = #id_ty;
            const QUAL_TABLE: &'static str = #qual_table;
            const COLS: &'static [&'static str] = &[ #( #cols_arr ),* ];
            const PK_COLS: &'static [&'static str] = &[ #( #pk_arr ),* ];
        }

        impl shl_sqlx::postgres::Readable for #ident {
            const SQL_SELECT_BY_PK: &'static str = #select_lit;
            const SQL_DELETE_BY_PK: &'static str = #delete_lit;

            async fn find_by_id<'e, E>(exec: E, id: <Self as shl_sqlx::postgres::TableMeta>::Id) -> Result<Self, shl_sqlx::postgres::CrudError>
            where E: sqlx::Executor<'e, Database = sqlx::Postgres> + Send {
                #bind_select
                let row = q.fetch_one(exec).await?;
                Ok(row)
            }

            async fn delete_by_id<'e, E>(exec: E, id: <Self as shl_sqlx::postgres::TableMeta>::Id) -> Result<u64, shl_sqlx::postgres::CrudError>
            where E: sqlx::Executor<'e, Database = sqlx::Postgres> + Send {
                #bind_delete
                let res = q.execute(exec).await?;
                Ok(res.rows_affected())
            }
        }
    };
    expanded.into()
}

#[proc_macro_error]
#[proc_macro_derive(Insertable, attributes(table))]
pub fn derive_insertable(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let ident = input.ident.clone();

    let cfg = match parse_model_cfg(&input.attrs, &ident) {
        Ok(c) => c,
        Err(e) => return e.into_compile_error().into(),
    };

    let (cols, _rs_names, cols_sql, _pk_idents, _pk_types) = collect(&input, &cfg);

    let insert_cols: Vec<_> = cols_sql
        .iter()
        .filter(|c| !cfg.insert_skip.iter().any(|s| s == &unquote(c)))
        .cloned()
        .collect();
    let insert_fields: Vec<Ident> = cols
        .iter()
        .filter(|ci| !cfg.insert_skip.iter().any(|s| s == &unquote(&ci.sql_quoted)))
        .map(|ci| ci.rs_ident.clone())
        .collect();

    let qual_table = format!("\"{}\".\"{}\"", cfg.schema, cfg.table);
    let sql_insert = if insert_cols.is_empty() {
        format!("INSERT INTO {} DEFAULT VALUES", qual_table)
    } else {
        format!(
            "INSERT INTO {} ({} ) VALUES ({})",
            qual_table,
            insert_cols.join(", "),
            placeholders(insert_cols.len())
        )
    };
    let sql_insert_lit = syn::LitStr::new(&sql_insert, input.span());

    let insert_cols_arr = insert_cols.iter().map(|c| syn::LitStr::new(c, input.span()));
    let bind_fields = insert_fields.iter().map(|f| quote! { q = q.bind(&self.#f); });

    let expanded = quote! {
        impl shl_sqlx::postgres::Insertable for #ident {
            const INSERT_COLS: &'static [&'static str] = &[ #( #insert_cols_arr ),* ];
            const SQL_INSERT: &'static str = #sql_insert_lit;

            async fn insert<'e, E>(&self, exec: E) -> Result<u64, shl_sqlx::postgres::CrudError>
            where E: sqlx::Executor<'e, Database = sqlx::Postgres> + Send {
                let mut q = sqlx::query(Self::SQL_INSERT);
                #( #bind_fields )*
                let res = q.execute(exec).await?;
                Ok(res.rows_affected())
            }
        }
    };
    expanded.into()
}

#[proc_macro_error]
#[proc_macro_derive(Updatable, attributes(table))]
pub fn derive_updatable(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let ident = input.ident.clone();

    let cfg = match parse_model_cfg(&input.attrs, &ident) {
        Ok(c) => c,
        Err(e) => return e.into_compile_error().into(),
    };
    let (cols, _rs_names, cols_sql, pk_idents, _pk_types) = collect(&input, &cfg);

    let upd_cols: Vec<&ColInfo> = cols
        .iter()
        .filter(|ci| {
            !cfg.skip_update
                .iter()
                .any(|s| s == &ci.rs_ident.to_string() || s == &unquote(&ci.sql_quoted))
        })
        .collect();

    if upd_cols.is_empty() {
        abort!(input.span(), "no fields to UPDATE (all are in skip_update)");
    }

    let set_list: Vec<String> = upd_cols
        .iter()
        .enumerate()
        .map(|(i, ci)| format!("{} = ${}", ci.sql_quoted, i + 1))
        .collect();

    let qual_table = format!("\"{}\".\"{}\"", cfg.schema, cfg.table);

    let mut where_s = String::new();
    for (i, pk) in cfg.pk_cols.iter().enumerate() {
        if i > 0 {
            where_s.push_str(" AND ");
        }
        where_s.push_str(&format!("\"{}\" = ${}", pk, i + upd_cols.len() + 1));
    }
    let sql_update = format!("UPDATE {} SET {} WHERE {}", qual_table, set_list.join(", "), where_s);
    let sql_update_lit = syn::LitStr::new(&sql_update, input.span());

    let bind_upd = upd_cols.iter().map(|ci| {
        let id = ci.rs_ident.clone();
        quote! { q = q.bind(&self.#id); }
    });

    let bind_pk = match pk_idents.len() {
        1 => {
            let a = pk_idents[0].clone();
            quote! { q = q.bind(&self.#a); }
        }
        2 => {
            let a = pk_idents[0].clone();
            let b = pk_idents[1].clone();
            quote! { q = q.bind(&self.#a); q = q.bind(&self.#b); }
        }
        _ => abort!(input.span(), "only 1-2 PK columns are supported"),
    };

    let expanded = quote! {
        impl shl_sqlx::postgres::Updatable for #ident {
            const SQL_UPDATE: &'static str = #sql_update_lit;

            async fn update<'e, E>(&self, exec: E) -> Result<u64, shl_sqlx::postgres::CrudError>
            where E: sqlx::Executor<'e, Database = sqlx::Postgres> + Send {
                let mut q = sqlx::query(Self::SQL_UPDATE);
                #( #bind_upd )*
                #bind_pk
                let res = q.execute(exec).await?;
                Ok(res.rows_affected())
            }
        }
    };
    expanded.into()
}
