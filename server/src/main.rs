#![warn(
    deprecated_in_future,
    macro_use_extern_crate,
    missing_debug_implementations,
    unused_qualifications
)]

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpResponse, HttpServer, ResponseError};
use anyhow::{format_err, Context, Error};
use bb8_postgres::{bb8, PostgresConnectionManager};
use chrono::{DateTime, Utc};
use derive_more::Display;
use futures::prelude::*;
use indoc::indoc;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::env;
use tokio_postgres::{SimpleQueryMessage, SimpleQueryRow};

type Pool = bb8::Pool<PostgresConnectionManager<tokio_postgres::tls::NoTls>>;

struct AppState {
    pool: Pool,
}

#[derive(Deserialize)]
struct Params {
    query: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum Response {
    #[serde(rename_all = "camelCase")]
    Success {
        last_updated: String,
        column_names: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Error(String),
}

#[actix_rt::main]
async fn main() -> Result<(), Error> {
    color_backtrace::install();

    dotenv::dotenv().ok();
    let database_url =
        env::var("DATABASE_URL").context("Environment variable DATABASE_URL is not set")?;
    let port = env::var("PORT")
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
        .unwrap_or(11265);

    let pool = establish_database_connection(&database_url)
        .await
        .expect("Failed to establish connection to the database");
    let app_state = web::Data::new(AppState { pool });

    let server = HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .wrap(middleware::Compress::default())
            .route("/", web::get().to(query))
            .wrap(Cors::permissive())
    })
    .bind(format!("0.0.0.0:{}", port))
    .unwrap()
    .run();

    println!("Listening on port {}", port);

    server.await?;

    Ok(())
}

async fn establish_database_connection(database_url: &str) -> Result<Pool, Error> {
    let manager =
        PostgresConnectionManager::new_from_stringlike(database_url, tokio_postgres::tls::NoTls)?;
    let pool = Pool::builder().build(manager).await.unwrap();

    Ok(pool)
}

// TODO: return error response where appropriate
fn query(
    state: web::Data<AppState>,
    params: web::Query<Params>,
) -> impl Future<Output = Result<HttpResponse, HandlerError>> {
    let fut = async move {
        let conn = state.pool.get().await?;
        let last_updated = {
            let rows = conn.query("SELECT last_updated FROM metadata", &[]).await?;

            match rows.first() {
                Some(row) => row.get::<_, DateTime<Utc>>(0).to_rfc3339(),
                None => {
                    return Ok(HttpResponse::ServiceUnavailable().finish());
                }
            }
        };

        let mut q1 = String::from(indoc!(
            "
            BEGIN;
            CREATE TEMP TABLE tmp
                ON COMMIT DROP
            AS
            "
        ));
        q1.push_str(&params.query);

        if let Err(e) = conn.batch_execute(&q1).await {
            conn.batch_execute("COMMIT").await?;

            return Ok(HttpResponse::Ok().json(Response::Error(format!("{}", e))));
        }

        let q2 = indoc!(
            "
            SELECT attname
            FROM pg_attribute
            WHERE attrelid = 'tmp'::regclass
              AND attnum > 0
              AND NOT attisdropped
            ORDER BY attnum
            "
        );

        let column_names = match conn.simple_query(q2).await {
            Ok(resp) => read_column_names(&resp)?,
            Err(e) => {
                return Ok(HttpResponse::Ok().json(Response::Error(format!("{}", e))));
            }
        };

        let q3 = indoc!(
            "
            SELECT * FROM tmp;
            COMMIT;
            "
        );

        let rows = match conn.simple_query(q3).await {
            Ok(resp) => read_rows(&resp),
            Err(e) => {
                return Ok(HttpResponse::Ok().json(Response::Error(format!("{}", e))));
            }
        };

        Ok(HttpResponse::Ok().json(Response::Success {
            last_updated,
            column_names,
            rows,
        }))
    };

    fut.boxed()
}

fn read_rows(query_response: &[SimpleQueryMessage]) -> Vec<Vec<String>> {
    first_query_rows(query_response)
        .map(row_to_strings)
        .collect()
}

fn read_column_names(query_response: &[SimpleQueryMessage]) -> Result<Vec<String>, Error> {
    first_query_rows(query_response)
        .map(|row: &SimpleQueryRow| match row.try_get(0) {
            Ok(Some(value)) => Ok(value.to_string()),
            Ok(None) => Err(format_err!("Column query response is invalid")),
            Err(e) => Err(Error::from(e).context("Column query response is invalid")),
        })
        .collect()
}

fn first_query_rows(
    query_response: &[SimpleQueryMessage],
) -> impl Iterator<Item = &SimpleQueryRow> {
    query_response
        .iter()
        .map(|msg| {
            if let SimpleQueryMessage::Row(row) = msg {
                Some(row)
            } else {
                None
            }
        })
        .while_some()
}

fn row_to_strings(row: &SimpleQueryRow) -> Vec<String> {
    let mut row_response = Vec::with_capacity(row.len());
    for i in 0..row.len() {
        row_response.push(row.get(i).unwrap_or("").to_string())
    }

    row_response
}

#[derive(Debug, Display)]
struct HandlerError(Error);

impl<E> From<E> for HandlerError
where
    E: Into<Error>,
{
    fn from(error: E) -> Self {
        HandlerError(error.into())
    }
}

impl ResponseError for HandlerError {}
