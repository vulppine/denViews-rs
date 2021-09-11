use crypto::digest::Digest;
use crypto::sha3::Sha3;
use r2d2::Pool;
use r2d2_postgres::{postgres::{config::Config, NoTls, Row}, PostgresConnectionManager};
use std::time::SystemTime;
use super::Error;

pub struct DatabaseClient {
    db_pool: Pool<PostgresConnectionManager<NoTls>>,
}

pub enum DatabaseOperation {
    // GET: Gets a page's views by string path.
    // If the record does not exist, this will always return an error.
    Get(String),

    // UPDATE: Updates a page's views by string path.
    //
    // If the record does not exist, this will always return an error.
    // Records should be tested for correctness before calling it
    // into the database.
    //
    // This will, as of v0.1, only increment views.
    UpdatePage(String, String),

    /*
    // CREATE: Creates a new page in the database.
    //
    // This is to resolve the two errors from above. Checking for
    // correctness must be done by the caller.
    CreatePage(String),
    */

    // FLUSH: This flushes the page_visitors table to the database,
    // calculating all required values and adding them to the record,
    // before deleting all related records from the page_visitors table.
    //
    // This level of denormalization is required for performance, as otherwise
    // you would have to deal with querying n rows for several pages
    // in the worst case (it is practically O(n) to calculate views from the
    // database from page_visitors alone).
    //
    // Due to the length of time it could take to flush records to database,
    // compared to fetching/updating, this should only be done by authorized
    // clients/callers in order to ensure that the database is not overloaded
    // with concurrent transactions.
    Flush,
}

pub struct ViewRecord {
    pub page: String,
    pub views: i64,
    pub hits: i64,
}

impl DatabaseClient {
    pub fn new(pool_size: u32, config: Config) -> Result<Self, Error> {
        Ok(DatabaseClient {
            db_pool: Pool::builder()
                .max_size(pool_size)
                .build(PostgresConnectionManager::new(config, NoTls))?,
        })
    }

    pub fn execute(&self, op: DatabaseOperation) -> Result<Option<ViewRecord>, Error> {
        match op {
            DatabaseOperation::Get(path) => Ok(Some(self.get_page_info(path)?)),
            DatabaseOperation::UpdatePage(path, ip) => {
                self.append_visitor(path, ip)?;
                Ok(None)
            }
            /*
            DatabaseOperation::CreatePage(path) => {
                self.create_page(path)?;
                Ok(None)
            },
            */
            DatabaseOperation::Flush => Ok(None),
        }
    }

    fn get_page_info(&self, path: String) -> Result<ViewRecord, Error> {
        let conn = self.db_pool.get()?;

        // this should ONLY RETURN a single row, ALWAYS
        // any other result is completely wrong

        let path_id: i32 = conn
            .query_one("SELECT path_id FROM paths WHERE path = $1", &[&path])?
            .get(0);
        let record = conn.query_one(
            "SELECT view_count, hit_count FROM path_$1",
            &[&path_id],
        )?;

        Ok(ViewRecord {
            page: path,
            views: record.get(0),
            hits: record.get(1),
        })
    }

    fn append_visitor(&self, path: String, visitor_info: String) -> Result<(), Error> {
        let conn = self.db_pool.get()?;

        // either get a page ID, or create it
        let page_id: i32 = conn
            .query_opt("SELECT page_id FROM pages WHERE path_id = (SELECT path_id FROM paths WHERE path = $1)", &[&path])?
            .unwrap_or(self.create_page(path)?)
            .get(0);

        let mut hasher = Sha3::sha3_256();
        let salt: &str = conn.query_one("SELECT hash FROM hash", &[])?.get(0);
        hasher.input_str(&(visitor_info + salt));
        let visitor_hash = hasher.result_str();

        // optional! this is because if the visitor doesn't already exist, it is instead
        // added into the visitors table
        let visitor = conn.query_opt(
            "SELECT visitor_id from visitors WHERE visitor_id = $1",
            &[&visitor_hash],
        )?;

        if let Some(v) = visitor {
            let id: i64 = v.get(0);
            let page_visitor = conn
                .query_opt("SELECT visitor_id, page_id, visitor_hits from page_visitors WHERE visitor_id = $1 AND page_id = $2",
                    &[&id, &page_id])?;

            if let Some(p) = page_visitor {
                let hits: i64 = p.get(2);
                conn.execute("UPDATE page_visitors SET visitor_hits = $3 WHERE visitor_id = $1, page_id = $2",
                    &[&id, &page_id, &(hits + 1)])?;
            } else {
                conn.execute(
                    "INSERT INTO page_visitors (visitor_id, page_id) VALUES ($1, $2)",
                    &[&id, &page_id],
                )?;
            }
        } else {
            conn.execute("INSERT INTO visitors (visitor_id) VALUES ($1)", &[&visitor_hash])?;
            conn.execute(
                "INSERT INTO page_visitors (visitor_id, page_id) VALUES ($1, $2)",
                &[&visitor_hash, &page_id],
            )?;
        }

        Ok(())
    }

    // This will create a page record, as well as any path categories
    // that eventually lead to the page record itself.
    //
    // If the path length is one, however, it will just create a page record,
    // and assume that the path category is the root of the website.
    fn create_page(&self, path: String) -> Result<Row, Error> {
        let conn = self.db_pool.get()?;

        let row = conn
            .query_opt(
                "SELECT * FROM pages WHERE path_id = (SELECT * FROM paths WHERE path = $1)",
                &[&path],
            )?;

        if let Some(r) = row {
            return Ok(r);
        }

        let path_id: i64 = conn
            .query_one("INSERT INTO paths (path) VALUES ($1) RETURNING path_id", &[&path])?
            .get(0);

        let parts = path[1..].split('/').collect::<Vec<&str>>();
        /*
        if parts.len() == 1 {
                conn.execute(
                "INSERT INTO pages VALUES (0, $1, $2, $3)",
                &[&path_id.to_string(), &parts[0], &"date"],
            )?;
            return Ok(());
        }
        */

        let mut last_part_id = 0i64;
        for part in parts[..parts.len() - 1].iter() {
            let folder: Option<Row> = conn.query_opt("SELECT folder_id, parent_id FROM folders WHERE folder_name = $1 AND parent_id = $2", &[&part, &last_part_id])?;
            if let Some(r) = folder {
                last_part_id = r.get(0);
                continue;
            }

            last_part_id = conn.query_one("INSERT INTO folders (folder_name) VALUES ($1) RETURNING folder_id", &[&part])?.get(0);
        }

        let row = conn.query_one("INSERT INTO pages (folder_id, path_id, page_name, first_visited) VALUES ($1, $2, $3, $4) RETURNING page_id",
        &[&last_part_id.to_string(), &path_id, &parts[parts.len() - 1], &SystemTime::now()])?;

        conn.execute("CREATE VIEW path_$1 (page_id, view_count, hit_count) AS SELECT view_count.page_id, view_count.views + total_views, view_count.hits + total_hits FROM (SELECT page_id, COUNT(visitor_id) AS views, SUM(visitor_hits) AS hits FROM page_visitors GROUP BY page_id WHERE page_id = $2) AS view_count LEFT JOIN pages ON view_count.page_id = pages.page_id", &[&path_id, &row.get::<usize, i64>(0)])?;

        Ok(row)
    }
}
