use std::env;

use base64;
use chrono::{DateTime, Utc};
use flate2::{write::GzEncoder, Compression};
use postgres::{Connection, TlsMode};
use serde::{Deserialize, Serialize, Serializer};
use serde_json;
use snafu::{ResultExt, Snafu};
use warp::{
    http::{self, Response},
    reply,
};

#[derive(Serialize)]
pub struct LogoState {
    time: DateTime<Utc>,
    #[serde(serialize_with = "as_base64")]
    logo: Vec<u8>,
}

fn as_base64<T, S>(key: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: AsRef<[u8]>,
    S: Serializer,
{
    serializer.serialize_str(&base64::encode(key.as_ref()))
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Could not get environment variable {}", env))]
    EnvVar {
        env: String,
        source: env::VarError,
    },
    #[snafu(display("PostgresError {}", source))]
    PgError {
        source: postgres::Error,
    },
    #[snafu(display("Error inserting {} into {}: {}", value, table, source))]
    PgInsert {
        table: String,
        value: String,
        source: postgres::Error,
    },
    PgQuery {
        query: String,
        source: postgres::Error,
    },
    JsonError {
        source: serde_json::Error,
    },
    HttpError {
        source: http::Error,
    },
    EncodeError {
        source: std::io::Error,
    },
    ParseDateError {
        source: chrono::format::ParseError,
    },
}

fn get_conn() -> Result<Connection, Error> {
    let db = std::env::var("DATABASE_URL").context(EnvVar {
        env: "DATABASE_URL",
    })?;
    Ok(Connection::connect(db, TlsMode::None).context(PgError)?)
}

pub fn init_db() -> Result<(), Error> {
    let conn = get_conn()?;

    let trans = conn.transaction().context(PgError)?;

    trans
        .execute(
            "CREATE TABLE IF NOT EXISTS timeline (
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW() PRIMARY KEY,
            image_png BYTEA NOT NULL
        )",
            &[],
        )
        .context(PgError)?;

    trans.commit().context(PgError)?;

    Ok(())
}

pub fn save_logo(logo_png: &[u8]) -> Result<(), Error> {
    let conn = get_conn()?;

    let trans = conn.transaction().context(PgError)?;

    trans
        .execute("INSERT INTO timeline (image_png) VALUES ($1)", &[&logo_png])
        .context(PgError)?;

    trans.commit().context(PgError)?;

    Ok(())
}

#[derive(Debug, Deserialize, Copy, Clone, Default)]
pub struct GetHistoryOptions {
    limit: Option<u32>,
}

pub fn get_history(options: GetHistoryOptions) -> Result<reply::Response, Error> {
    let mut query_str = "SELECT created_at, image_png FROM timeline ORDER BY created_at".to_owned();
    if let Some(limit) = options.limit {
        // NOTE: This is safe because we know that limit is a number
        query_str.push_str(&format!(" LIMIT {}", limit));
    }

    let conn = get_conn()?;
    let res = conn.query(&query_str, &[]).context(PgError)?;

    let data = res
        .into_iter()
        .map(|row| LogoState {
            time: row.get(0),
            logo: row.get(1),
        })
        .collect::<Vec<_>>();

    // TODO: Check if the browser accept gzip
    // let result = serde_json::to_vec(&data).context(JsonError)?;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    serde_json::to_writer(&mut encoder, &data).context(JsonError)?;

    let result = encoder.finish().context(EncodeError)?;

    Ok(Response::builder()
        .header("Content-Type", "application/json")
        .header("Content-Encoding", "gzip")
        .body(result.into())
        .context(HttpError)?)
}

pub fn get_history_from_date(index: String) -> Result<reply::Response, Error> {
    let date: DateTime<Utc> = index.parse().context(ParseDateError)?;

    let conn = get_conn()?;
    let res = conn
        .query(
            "SELECT image_png FROM timeline WHERE created_at=$1",
            &[&date],
        )
        .context(PgError)?;

    let data: Vec<u8> = res.get(0).get(0);

    Ok(Response::builder()
        .header("Content-Type", "image/png")
        .body(data.into())
        .context(HttpError)?)
}

#[derive(Serialize)]
pub struct HistoryIndex {
    time: DateTime<Utc>,
}

pub fn get_history_index() -> Result<reply::Response, Error> {
    let conn = get_conn()?;

    let res = conn
        .query("SELECT created_at FROM timeline ORDER BY created_at", &[])
        .context(PgError)?;

    let data = res
        .into_iter()
        .map(|row| HistoryIndex { time: row.get(0) })
        .collect::<Vec<_>>();

    let result = serde_json::to_string(&data).context(JsonError)?;

    Ok(Response::builder()
        .header("Content-Type", "application/json")
        .body(result.into())
        .context(HttpError)?)
}
