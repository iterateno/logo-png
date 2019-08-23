use std::env;

use postgres::{Connection, TlsMode};
use snafu::{ResultExt, Snafu};

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
