use std::env;

use base64;
use chrono::{DateTime, Utc};
use postgres::{Connection, TlsMode};
use serde::{Serialize, Serializer};
use snafu::{ResultExt, Snafu};

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
    EnvVar { env: String, source: env::VarError },
    #[snafu(display("PostgresError {}", source))]
    PgError { source: postgres::Error },
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

pub fn get_history() -> Result<Vec<LogoState>, Error> {
    let conn = get_conn()?;
    let res = conn
        .query(
            "SELECT created_at, image_png FROM timeline ORDER BY created_at",
            &[],
        )
        .context(PgError)?;

    Ok(res
        .into_iter()
        .map(|row| LogoState {
            time: row.get(0),
            logo: row.get(1),
        })
        .collect())
}
