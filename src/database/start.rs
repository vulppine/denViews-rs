use super::{mariadb, postgres};
use crate::Error;

#[cfg(feature = "postgres")]
pub async fn start_db() -> Result<(postgres::Postgres, postgres::PostgresDatabaseTools), Error> {
    let db = postgres::database::Postgres::new().await?;
    let db_tools = postgres::PostgresDatabaseTools::new().await?;

    Ok((db, db_tools))
}

#[cfg(feature = "mariadb")]
pub async fn start_db() -> Result<(mariadb::MariaDB, mariadb::MariaDBDatabaseTools), Error> {
    let db = mariadb::database::MariaDB::new().await?;
    let db_tools = mariadb::MariaDBDatabaseTools::new().await?;

    Ok((db, db_tools))
}
