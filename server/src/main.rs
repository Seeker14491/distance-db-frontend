#![warn(
    deprecated_in_future,
    macro_use_extern_crate,
    missing_debug_implementations,
    unused_qualifications
)]

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpResponse, HttpServer, ResponseError};
use anyhow::{Context, Error};
use bb8_postgres::{bb8, PostgresConnectionManager};
use chrono::{DateTime, Utc};
use derive_more::Display;
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
    .bind(format!("0.0.0.0:{}", port))?
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
async fn query(
    state: web::Data<AppState>,
    params: web::Query<Params>,
) -> Result<HttpResponse, HandlerError> {
    let conn = state.pool.get().await?;
    let last_updated = {
        let rows = conn.query("SELECT last_updated FROM metadata", &[]).await?;

        if let Some(row) = rows.first() {
            row.get::<_, DateTime<Utc>>(0).to_rfc3339()
        } else {
            return Ok(HttpResponse::ServiceUnavailable().finish());
        }
    };

    let resp = match conn.simple_query(&params.query).await {
        Ok(resp) => resp,
        Err(e) => {
            return Ok(HttpResponse::Ok().json(Response::Error(format!("{}", e))));
        }
    };

    let mut column_names = Vec::new();
    if let Some(SimpleQueryMessage::RowDescription(cols)) = resp.first() {
        column_names = cols.iter().map(|col| col.name().to_string()).collect();
    };

    let rows = read_rows(&resp);

    Ok(HttpResponse::Ok().json(Response::Success {
        last_updated,
        column_names,
        rows,
    }))
}

fn read_rows(query_response: &[SimpleQueryMessage]) -> Vec<Vec<String>> {
    query_response
        .iter()
        .filter_map(|msg| {
            if let SimpleQueryMessage::Row(row) = msg {
                Some(row_to_strings(row))
            } else {
                None
            }
        })
        .collect()
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
