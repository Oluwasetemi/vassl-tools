use anyhow::Context as _;
use sqlez::domain::Domain;
use vassl_core::{Project, ProjectStatus, QuotationItem, QuotationStatus};
use vassl_db::SharedDomain;

pub struct QuotationDb(pub sqlez::thread_safe_connection::ThreadSafeConnection);

impl Domain for QuotationDb {
    const NAME: &'static str = "quotations";
    const MIGRATIONS: &'static [&'static str] = &[
        // projects is owned by SharedDomain; we create it here so that the
        // sqlez FK-cleanup pass can resolve the REFERENCES projects(id)
        // constraint on quotations.
        "CREATE TABLE IF NOT EXISTS projects (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            client_name TEXT NOT NULL,
            description TEXT,
            status      TEXT NOT NULL DEFAULT 'active',
            created_at  TEXT NOT NULL
        )",
        // products is owned by InventoryDb; stub here for FK-cleanup on quotation_items.
        "CREATE TABLE IF NOT EXISTS products (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            sku             TEXT UNIQUE NOT NULL,
            name            TEXT NOT NULL,
            category        TEXT,
            unit            TEXT NOT NULL,
            min_stock_level REAL NOT NULL DEFAULT 0,
            notes           TEXT,
            created_at      TEXT NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS quotations (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id       INTEGER NOT NULL REFERENCES projects(id),
            reference_number TEXT UNIQUE NOT NULL,
            status           TEXT NOT NULL DEFAULT 'draft',
            notes            TEXT,
            created_by       TEXT NOT NULL,
            created_at       TEXT NOT NULL,
            updated_at       TEXT NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS quotation_items (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            quotation_id   INTEGER NOT NULL REFERENCES quotations(id),
            product_id     INTEGER REFERENCES products(id),
            description    TEXT NOT NULL,
            quantity       REAL NOT NULL,
            unit_price_usd REAL NOT NULL,
            total_usd      REAL NOT NULL
        )",
    ];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool { false }
}

vassl_db::static_connection!(QuotationDb, [SharedDomain]);

#[derive(Debug, Clone)]
pub struct QuotationRow {
    pub id:               i64,
    pub reference_number: String,
    pub status:           QuotationStatus,
    pub project_id:       i64,
    pub project_name:     String,
    pub client_name:      String,
    pub total_usd:        f64,
    pub created_at:       String,
    pub notes:            Option<String>,
}

fn status_from_str(s: &str) -> QuotationStatus {
    match s {
        "sent"     => QuotationStatus::Sent,
        "accepted" => QuotationStatus::Accepted,
        "rejected" => QuotationStatus::Rejected,
        _          => QuotationStatus::Draft,
    }
}

fn status_to_str(s: &QuotationStatus) -> &'static str {
    match s {
        QuotationStatus::Draft    => "draft",
        QuotationStatus::Sent     => "sent",
        QuotationStatus::Accepted => "accepted",
        QuotationStatus::Rejected => "rejected",
    }
}

impl QuotationDb {
    pub fn next_reference_number(&self) -> anyhow::Result<String> {
        let year    = chrono::Utc::now().format("%Y").to_string();
        let pattern = format!("VASSL-{year}-%");
        let count: i64 = self
            .select_row_bound::<String, Option<i64>>(
                "SELECT COUNT(*) FROM quotations WHERE reference_number LIKE ?1",
            )
            .context("prepare count")?
            (pattern)
            .context("execute count")?
            .flatten()
            .unwrap_or(0);
        Ok(format!("VASSL-{year}-{:04}", count + 1))
    }

    pub fn list_quotations_with_project(&self) -> anyhow::Result<Vec<QuotationRow>> {
        // sqlez supports up to 10-tuples; select exactly 9 columns
        type Row = (i64, String, String, String, Option<String>, i64, String, String, f64);
        self.select::<Row>(
            "SELECT q.id, q.reference_number, q.status, q.created_at, q.notes,
                    q.project_id,
                    p.name, p.client_name,
                    COALESCE(SUM(i.total_usd), 0.0) AS total_usd
             FROM quotations q
             JOIN projects p ON p.id = q.project_id
             LEFT JOIN quotation_items i ON i.quotation_id = q.id
             GROUP BY q.id
             ORDER BY q.created_at DESC",
        )
        .context("prepare list_quotations_with_project")?()
        .context("execute list_quotations_with_project")
        .map(|rows| {
            rows.into_iter().map(|(id, ref_num, status_str, created_at, notes,
                                   project_id, project_name, client_name, total_usd)| {
                QuotationRow {
                    id,
                    reference_number: ref_num,
                    status: status_from_str(&status_str),
                    project_id,
                    project_name,
                    client_name,
                    total_usd,
                    created_at,
                    notes,
                }
            }).collect()
        })
    }

    pub fn list_items_for_quotation(&self, quotation_id: i64) -> anyhow::Result<Vec<QuotationItem>> {
        self.select_bound::<i64, (i64, i64, Option<i64>, String, f64, f64, f64)>(
            "SELECT id, quotation_id, product_id, description, quantity, unit_price_usd, total_usd
             FROM quotation_items WHERE quotation_id = ?1
             ORDER BY id ASC",
        )
        .context("prepare list_items_for_quotation")?
        (quotation_id)
        .context("execute list_items_for_quotation")
        .map(|rows| {
            rows.into_iter().map(|(id, qid, product_id, description, quantity, unit_price_usd, total_usd)| {
                QuotationItem { id, quotation_id: qid, product_id, description, quantity, unit_price_usd, total_usd }
            }).collect()
        })
    }

    pub fn list_projects(&self) -> anyhow::Result<Vec<Project>> {
        self.select::<(i64, String, String, Option<String>, String, String)>(
            "SELECT id, name, client_name, description, status, created_at
             FROM projects ORDER BY name",
        )
        .context("prepare list_projects")?()
        .context("execute list_projects")
        .map(|rows| {
            rows.into_iter().map(|(id, name, client_name, description, status_str, created_at)| {
                let status = match status_str.as_str() {
                    "completed" => ProjectStatus::Completed,
                    "archived"  => ProjectStatus::Archived,
                    _           => ProjectStatus::Active,
                };
                Project { id, name, client_name, description, status, created_at }
            }).collect()
        })
    }

    pub async fn insert_quotation(
        &self,
        project_id:       i64,
        reference_number: impl Into<String>,
        created_by:       impl Into<String>,
    ) -> anyhow::Result<i64> {
        let ref_num    = reference_number.into();
        let created_by = created_by.into();
        let now        = chrono::Utc::now().to_rfc3339();

        self.write(move |conn| {
            conn.exec_bound::<(i64, String, String, String, String)>(
                "INSERT INTO quotations
                 (project_id, reference_number, status, created_by, created_at, updated_at)
                 VALUES (?1, ?2, 'draft', ?3, ?4, ?5)",
            )
            .context("prepare insert_quotation")?
            ((project_id, ref_num, created_by, now.clone(), now))
            .context("execute insert_quotation")?;

            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare rowid")?()
                .context("execute rowid")?
                .context("rowid was None")
        })
        .await
    }

    pub async fn insert_quotation_with_notes(
        &self,
        project_id:       i64,
        reference_number: impl Into<String>,
        created_by:       impl Into<String>,
        notes:            Option<&str>,
    ) -> anyhow::Result<i64> {
        let ref_num    = reference_number.into();
        let created_by = created_by.into();
        let notes      = notes.map(String::from);
        let now        = chrono::Utc::now().to_rfc3339();

        self.write(move |conn| {
            conn.exec_bound::<(i64, String, Option<String>, String, String, String)>(
                "INSERT INTO quotations
                 (project_id, reference_number, notes, created_by, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .context("prepare insert_quotation_with_notes")?
            ((project_id, ref_num, notes, created_by, now.clone(), now))
            .context("execute insert_quotation_with_notes")?;

            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare rowid")?()
                .context("execute rowid")?
                .context("rowid was None")
        })
        .await
    }

    pub async fn insert_project(&self, name: String, client_name: String) -> anyhow::Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        self.write(move |conn| {
            conn.exec_bound::<(String, String, String)>(
                "INSERT INTO projects (name, client_name, status, created_at) VALUES (?1, ?2, 'active', ?3)",
            )
            .context("prepare insert_project")?
            ((name, client_name, now))
            .context("execute insert_project")?;
            conn.select_bound::<(), i64>("SELECT last_insert_rowid()")
                .context("prepare last_insert_rowid")?
                (())
                .context("execute last_insert_rowid")?
                .into_iter().next().ok_or_else(|| anyhow::anyhow!("no rowid"))
        })
        .await
    }

    pub async fn update_status(&self, id: i64, status: QuotationStatus) -> anyhow::Result<()> {
        let status_str = status_to_str(&status).to_string();
        let now        = chrono::Utc::now().to_rfc3339();

        self.write(move |conn| {
            conn.exec_bound::<(String, String, i64)>(
                "UPDATE quotations SET status = ?1, updated_at = ?2 WHERE id = ?3",
            )
            .context("prepare update_status")?
            ((status_str, now, id))
            .context("execute update_status")
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_project(db: &QuotationDb, name: &str, client: &str) -> i64 {
        let name   = name.to_string();
        let client = client.to_string();
        db.write(move |conn| {
            conn.exec(
                "CREATE TABLE IF NOT EXISTS projects (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    name        TEXT NOT NULL,
                    client_name TEXT NOT NULL,
                    description TEXT,
                    status      TEXT NOT NULL DEFAULT 'active',
                    created_at  TEXT NOT NULL
                )",
            ).context("create projects table")?()?;
            conn.exec_bound::<(String, String)>(
                "INSERT INTO projects (name, client_name, created_at)
                 VALUES (?1, ?2, datetime('now'))",
            ).context("prepare insert")?((name, client)).context("exec insert")?;
            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare rowid")?()?
                .context("rowid None")
        }).await.unwrap()
    }

    #[tokio::test]
    async fn next_reference_number_starts_at_0001() {
        let db  = QuotationDb::open_test_db("quot_ref_first").await;
        let ref_num = db.next_reference_number().unwrap();
        let year = chrono::Utc::now().format("%Y").to_string();
        assert_eq!(ref_num, format!("VASSL-{year}-0001"));
    }

    #[tokio::test]
    async fn next_reference_number_increments() {
        let db  = QuotationDb::open_test_db("quot_ref_incr").await;
        let pid = setup_project(&db, "P1", "C1").await;
        db.insert_quotation(pid, "VASSL-2026-0001", "tester").await.unwrap();
        let ref_num = db.next_reference_number().unwrap();
        assert_eq!(ref_num, "VASSL-2026-0002");
    }

    #[tokio::test]
    async fn insert_and_list_quotation() {
        let db  = QuotationDb::open_test_db("quot_insert_list").await;
        let pid = setup_project(&db, "Project A", "Client A").await;
        let id  = db.insert_quotation(pid, "VASSL-2026-0001", "alice").await.unwrap();
        assert!(id > 0);
        let rows = db.list_quotations_with_project().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].reference_number, "VASSL-2026-0001");
        assert_eq!(rows[0].status, QuotationStatus::Draft);
        assert_eq!(rows[0].project_name, "Project A");
    }

    #[tokio::test]
    async fn update_status_changes_quotation_status() {
        let db  = QuotationDb::open_test_db("quot_status_update").await;
        let pid = setup_project(&db, "P2", "C2").await;
        let id  = db.insert_quotation(pid, "VASSL-2026-0001", "alice").await.unwrap();
        db.update_status(id, QuotationStatus::Sent).await.unwrap();
        let rows = db.list_quotations_with_project().unwrap();
        assert_eq!(rows[0].status, QuotationStatus::Sent);
    }

    #[tokio::test]
    async fn list_items_empty_for_new_quotation() {
        let db  = QuotationDb::open_test_db("quot_items_empty").await;
        let pid = setup_project(&db, "P3", "C3").await;
        let qid = db.insert_quotation(pid, "VASSL-2026-0001", "bob").await.unwrap();
        let items = db.list_items_for_quotation(qid).unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn list_projects_returns_seeded_projects() {
        let db = QuotationDb::open_test_db("quot_list_projects").await;
        let _ = setup_project(&db, "Alpha", "AlphaCo").await;
        let _ = setup_project(&db, "Beta",  "BetaCo").await;
        let projects = db.list_projects().unwrap();
        assert_eq!(projects.len(), 2);
    }
}
