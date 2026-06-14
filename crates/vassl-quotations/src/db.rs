use anyhow::Context as _;
use sqlez::domain::Domain;
use vassl_core::{Project, ProjectStatus, QuotationExtras, QuotationItem, QuotationStatus};
use vassl_db::SharedDomain;
use vassl_inventory::db::InventoryDb;

pub struct QuotationDb(pub sqlez::thread_safe_connection::ThreadSafeConnection);

impl Domain for QuotationDb {
    const NAME: &'static str = "quotations";
    const MIGRATIONS: &'static [&'static str] = &[
        "CREATE TABLE IF NOT EXISTS projects (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            name           TEXT NOT NULL,
            client_name    TEXT NOT NULL,
            description    TEXT,
            status         TEXT NOT NULL DEFAULT 'active',
            created_at     TEXT NOT NULL,
            client_address TEXT,
            client_attn    TEXT,
            client_tel     TEXT,
            date_started   TEXT,
            date_completed TEXT,
            technicians    TEXT,
            client_contact TEXT,
            vassl_contact  TEXT,
            signedoff_date TEXT
        )",
        // products is owned by inventory; stub here so FK constraints resolve.
        "CREATE TABLE IF NOT EXISTS products (
            id                    INTEGER PRIMARY KEY AUTOINCREMENT,
            sku                   TEXT UNIQUE NOT NULL,
            name                  TEXT NOT NULL,
            category              TEXT,
            unit                  TEXT NOT NULL,
            min_stock_level       REAL NOT NULL DEFAULT 0,
            notes                 TEXT,
            created_at            TEXT NOT NULL,
            description           TEXT,
            preferred_supplier_id INTEGER,
            model_number          TEXT,
            part_number           TEXT,
            duty_percent          REAL NOT NULL DEFAULT 0,
            end_of_life           INTEGER NOT NULL DEFAULT 0,
            replacement           TEXT
        )",
        "CREATE TABLE IF NOT EXISTS quotations (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id       INTEGER NOT NULL REFERENCES projects(id),
            reference_number TEXT UNIQUE NOT NULL,
            status           TEXT NOT NULL DEFAULT 'draft',
            notes            TEXT,
            created_by       TEXT NOT NULL,
            created_at       TEXT NOT NULL,
            updated_at       TEXT NOT NULL,
            quotation_date   TEXT,
            exchange_rate_jmd REAL NOT NULL DEFAULT 0.0,
            discount_percent  REAL NOT NULL DEFAULT 0.0,
            gct_percent       REAL NOT NULL DEFAULT 15.0,
            validity_days     INTEGER NOT NULL DEFAULT 30
        )",
        "CREATE TABLE IF NOT EXISTS quotation_items (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            quotation_id     INTEGER NOT NULL REFERENCES quotations(id),
            product_id       INTEGER REFERENCES products(id),
            description      TEXT NOT NULL,
            quantity         REAL NOT NULL,
            unit_price_usd   REAL NOT NULL,
            total_usd        REAL NOT NULL,
            unit             TEXT,
            discount_percent REAL NOT NULL DEFAULT 0.0
        )",
    ];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool { true }
}

vassl_db::static_connection!(QuotationDb, [SharedDomain, InventoryDb]);

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
        let year   = chrono::Utc::now().format("%Y").to_string();
        let prefix = vassl_db::shared::get_setting(self, "quotations.prefix")
            .ok().flatten()
            .unwrap_or_else(|| "VASSL".to_string());
        let pattern = format!("{prefix}-{year}-%");
        let count: i64 = self
            .select_row_bound::<String, Option<i64>>(
                "SELECT COUNT(*) FROM quotations WHERE reference_number LIKE ?1",
            )
            .context("prepare count")?
            (pattern)
            .context("execute count")?
            .flatten()
            .unwrap_or(0);
        Ok(format!("{prefix}-{year}-{:04}", count + 1))
    }

    pub fn list_quotations_with_project(&self) -> anyhow::Result<Vec<QuotationRow>> {
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

    pub fn get_quotation_extras(&self, quotation_id: i64) -> anyhow::Result<QuotationExtras> {
        type Row = (f64, f64, f64, i64, Option<String>);
        let row = self
            .select_bound::<i64, Row>(
                "SELECT exchange_rate_jmd, discount_percent, gct_percent, validity_days, quotation_date
                 FROM quotations WHERE id = ?1",
            )
            .context("prepare get_quotation_extras")?
            (quotation_id)
            .context("execute get_quotation_extras")?
            .into_iter().next();

        Ok(match row {
            Some((rate, disc, gct, days, date)) => QuotationExtras {
                exchange_rate_jmd: rate,
                discount_percent:  disc,
                gct_percent:       gct,
                validity_days:     days,
                quotation_date:    date,
            },
            None => QuotationExtras::default(),
        })
    }

    pub fn list_items_for_quotation(&self, quotation_id: i64) -> anyhow::Result<Vec<QuotationItem>> {
        type Row = (i64, i64, Option<i64>, String, f64, Option<String>, f64, f64, f64);
        self.select_bound::<i64, Row>(
            "SELECT id, quotation_id, product_id, description, quantity,
                    unit, unit_price_usd, discount_percent, total_usd
             FROM quotation_items WHERE quotation_id = ?1
             ORDER BY id ASC",
        )
        .context("prepare list_items_for_quotation")?
        (quotation_id)
        .context("execute list_items_for_quotation")
        .map(|rows| {
            rows.into_iter().map(|(id, qid, product_id, description, quantity,
                                   unit, unit_price_usd, discount_percent, total_usd)| {
                QuotationItem {
                    id, quotation_id: qid, product_id,
                    description, quantity, unit,
                    unit_price_usd, discount_percent, total_usd,
                }
            }).collect()
        })
    }

    pub fn list_projects(&self) -> anyhow::Result<Vec<Project>> {
        type Row = (i64, String, String, Option<String>, Option<String>, Option<String>, Option<String>, String, String,
                    Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>);
        self.select::<Row>(
            "SELECT id, name, client_name, client_address, client_attn, client_tel,
                    description, status, created_at,
                    date_started, date_completed, technicians, client_contact, vassl_contact, signedoff_date
             FROM projects ORDER BY name",
        )
        .context("prepare list_projects")?()
        .context("execute list_projects")
        .map(|rows| {
            rows.into_iter().map(|(id, name, client_name, client_address, client_attn,
                                   client_tel, description, status_str, created_at,
                                   date_started, date_completed, technicians, client_contact, vassl_contact, signedoff_date)| {
                let status = match status_str.as_str() {
                    "completed" => ProjectStatus::Completed,
                    "archived"  => ProjectStatus::Archived,
                    _           => ProjectStatus::Active,
                };
                Project { id, name, client_name, client_address, client_attn, client_tel,
                          description, status, created_at,
                          date_started, date_completed, technicians, client_contact, vassl_contact, signedoff_date }
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

    pub async fn insert_quotation_atomic(
        &self,
        project_id:        i64,
        created_by:        impl Into<String>,
        notes:             Option<&str>,
        exchange_rate_jmd: f64,
        discount_percent:  f64,
        gct_percent:       f64,
        validity_days:     i64,
        quotation_date:    Option<&str>,
    ) -> anyhow::Result<i64> {
        let created_by     = created_by.into();
        let notes          = notes.map(String::from);
        let quotation_date = quotation_date.map(String::from);
        let now            = chrono::Utc::now().to_rfc3339();
        let year           = chrono::Utc::now().format("%Y").to_string();

        self.write(move |conn| {
            let pattern = format!("VASSL-{year}-%");
            let count: i64 = conn
                .select_bound::<String, i64>(
                    "SELECT COALESCE(COUNT(*), 0) FROM quotations WHERE reference_number LIKE ?1",
                )
                .context("prepare count")?
                (pattern)
                .context("execute count")?
                .into_iter().next().unwrap_or(0);
            let ref_num = format!("VASSL-{year}-{:04}", count + 1);

            conn.exec_bound::<(i64, String, Option<String>, String, String, String)>(
                "INSERT INTO quotations
                 (project_id, reference_number, notes, created_by, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .context("prepare insert_quotation_atomic")?
            ((project_id, ref_num, notes, created_by, now.clone(), now))
            .context("execute insert_quotation_atomic")?;

            let id: i64 = conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare rowid")?()
                .context("execute rowid")?
                .context("rowid was None")?;

            // Set financial fields separately to stay within sqlez tuple limits.
            conn.exec_bound::<(f64, f64, f64, i64, Option<String>, i64)>(
                "UPDATE quotations
                 SET exchange_rate_jmd = ?1,
                     discount_percent  = ?2,
                     gct_percent       = ?3,
                     validity_days     = ?4,
                     quotation_date    = ?5
                 WHERE id = ?6",
            )
            .context("prepare update_extras")?
            ((exchange_rate_jmd, discount_percent, gct_percent, validity_days, quotation_date, id))
            .context("execute update_extras")?;

            Ok(id)
        })
        .await
    }

    pub async fn insert_project(
        &self,
        name:           String,
        client_name:    String,
        client_address: Option<String>,
        client_attn:    Option<String>,
        client_tel:     Option<String>,
        date_started:   Option<String>,
        date_completed: Option<String>,
        technicians:    Option<String>,
        client_contact: Option<String>,
        vassl_contact:  Option<String>,
        signedoff_date: Option<String>,
    ) -> anyhow::Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        self.write(move |conn| {
            conn.exec_bound::<(String, String, Option<String>, Option<String>, Option<String>, String,
                               Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
                "INSERT INTO projects (name, client_name, client_address, client_attn, client_tel, status, created_at,
                                       date_started, date_completed, technicians, client_contact, vassl_contact, signedoff_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )
            .context("prepare insert_project")?
            ((name, client_name, client_address, client_attn, client_tel, now,
              date_started, date_completed, technicians, client_contact, vassl_contact, signedoff_date))
            .context("execute insert_project")?;
            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare last_insert_rowid")?()
                .context("execute last_insert_rowid")?
                .context("rowid was None")
        })
        .await
    }

    pub async fn update_project(
        &self,
        id:             i64,
        name:           String,
        client_name:    String,
        client_address: Option<String>,
        client_attn:    Option<String>,
        client_tel:     Option<String>,
        date_started:   Option<String>,
        date_completed: Option<String>,
        technicians:    Option<String>,
        client_contact: Option<String>,
        vassl_contact:  Option<String>,
        signedoff_date: Option<String>,
    ) -> anyhow::Result<()> {
        self.write(move |conn| {
            conn.exec_bound::<(String, String, Option<String>, Option<String>, Option<String>,
                               Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, i64)>(
                "UPDATE projects SET name=?1, client_name=?2, client_address=?3, client_attn=?4, client_tel=?5,
                                     date_started=?6, date_completed=?7, technicians=?8,
                                     client_contact=?9, vassl_contact=?10, signedoff_date=?11
                 WHERE id=?12"
            )
            .context("prepare update_project")?
            ((name, client_name, client_address, client_attn, client_tel,
              date_started, date_completed, technicians, client_contact, vassl_contact, signedoff_date, id))
            .context("execute update_project")
        })
        .await
    }

    pub async fn insert_item(
        &self,
        quotation_id:     i64,
        product_id:       Option<i64>,
        description:      String,
        quantity:         f64,
        unit:             Option<String>,
        unit_price_usd:   f64,
        discount_percent: f64,
        total_usd:        f64,
    ) -> anyhow::Result<i64> {
        self.write(move |conn| {
            conn.exec_bound::<(i64, Option<i64>, String, f64, Option<String>, f64, f64, f64)>(
                "INSERT INTO quotation_items
                 (quotation_id, product_id, description, quantity, unit, unit_price_usd, discount_percent, total_usd)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .context("prepare insert_item")?
            ((quotation_id, product_id, description, quantity, unit, unit_price_usd, discount_percent, total_usd))
            .context("execute insert_item")?;
            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare rowid")?()
                .context("execute rowid")?
                .context("rowid was None")
        })
        .await
    }

    pub async fn update_quotation_financials(
        &self,
        id:                i64,
        project_id:        i64,
        notes:             Option<String>,
        exchange_rate_jmd: f64,
        discount_percent:  f64,
        gct_percent:       f64,
        validity_days:     i64,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.write(move |conn| {
            conn.exec_bound::<(i64, Option<String>, String, i64)>(
                "UPDATE quotations SET project_id = ?1, notes = ?2, updated_at = ?3 WHERE id = ?4",
            )
            .context("prepare update_quotation project/notes")?
            ((project_id, notes, now, id))
            .context("execute update_quotation project/notes")?;

            conn.exec_bound::<(f64, f64, f64, i64, i64)>(
                "UPDATE quotations
                 SET exchange_rate_jmd = ?1, discount_percent = ?2, gct_percent = ?3, validity_days = ?4
                 WHERE id = ?5",
            )
            .context("prepare update_quotation financials")?
            ((exchange_rate_jmd, discount_percent, gct_percent, validity_days, id))
            .context("execute update_quotation financials")
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

    /// Atomically marks a quotation as Accepted and inserts a negative stock_entry for every
    /// line item that references a product (acquisition_type = 'project').
    pub async fn accept_quotation(&self, id: i64) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.write(move |conn| {
            conn.exec_bound::<(String, i64)>(
                "UPDATE quotations SET status = 'accepted', updated_at = ?1 WHERE id = ?2 AND status != 'accepted'",
            )
            .context("prepare accept_quotation update")?
            ((now.clone(), id))
            .context("execute accept_quotation update")?;

            type ItemRow = (i64, f64, Option<i64>);
            let items = conn
                .select_bound::<i64, ItemRow>(
                    "SELECT qi.product_id, qi.quantity, q.project_id
                     FROM quotation_items qi
                     JOIN quotations q ON q.id = qi.quotation_id
                     WHERE qi.quotation_id = ?1 AND qi.product_id IS NOT NULL",
                )
                .context("prepare accept items select")?
                (id)
                .context("execute accept items select")?;

            for (product_id, quantity, project_id) in items {
                conn.exec_bound::<(i64, f64, String, Option<i64>)>(
                    "INSERT INTO stock_entries
                     (product_id, quantity, unit_cost_usd, acquired_at, acquisition_type, project_id)
                     VALUES (?1, ?2, 0.0, ?3, 'project', ?4)",
                )
                .context("prepare stock deduct")?
                ((product_id, -quantity, now.clone(), project_id))
                .context("execute stock deduct")?;
            }

            Ok(())
        })
        .await
    }

    pub async fn delete_quotation(&self, id: i64) -> anyhow::Result<()> {
        self.write(move |conn| {
            conn.exec_bound::<i64>("DELETE FROM quotation_items WHERE quotation_id = ?1")
                .context("prepare delete quotation_items")?
                (id).context("execute delete quotation_items")?;
            conn.exec_bound::<i64>("DELETE FROM quotations WHERE id = ?1")
                .context("prepare delete quotation")?
                (id).context("execute delete quotation")?;
            Ok(())
        }).await
    }

    pub async fn delete_project(&self, id: i64) -> anyhow::Result<()> {
        self.write(move |conn| {
            conn.exec_bound::<i64>(
                "DELETE FROM quotation_items WHERE quotation_id IN (SELECT id FROM quotations WHERE project_id = ?1)"
            ).context("prepare cascade delete items")?
            (id).context("execute cascade delete items")?;
            conn.exec_bound::<i64>("DELETE FROM quotations WHERE project_id = ?1")
                .context("prepare delete quotations")?
                (id).context("execute delete quotations")?;
            conn.exec_bound::<i64>("DELETE FROM projects WHERE id = ?1")
                .context("prepare delete project")?
                (id).context("execute delete project")?;
            Ok(())
        }).await
    }

    /// Deletes a line item. If the parent quotation is accepted and the item had a product,
    /// inserts a positive stock_entry to reverse the prior deduction.
    pub async fn delete_item(&self, item_id: i64) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.write(move |conn| {
            type ItemInfo = (Option<i64>, f64, i64);
            let item = conn
                .select_bound::<i64, ItemInfo>(
                    "SELECT qi.product_id, qi.quantity, qi.quotation_id
                     FROM quotation_items qi WHERE qi.id = ?1",
                )
                .context("prepare delete_item select")?
                (item_id)
                .context("execute delete_item select")?
                .into_iter().next()
                .ok_or_else(|| anyhow::anyhow!("line item {item_id} not found"))?;

            let (product_id, quantity, quotation_id) = item;

            let status_str = conn
                .select_bound::<i64, String>(
                    "SELECT status FROM quotations WHERE id = ?1",
                )
                .context("prepare status select")?
                (quotation_id)
                .context("execute status select")?
                .into_iter().next()
                .unwrap_or_default();

            conn.exec_bound::<i64>("DELETE FROM quotation_items WHERE id = ?1")
                .context("prepare delete_item")?
                (item_id)
                .context("execute delete_item")?;

            if status_str == "accepted" {
                if let Some(pid) = product_id {
                    let project_id: Option<i64> = conn
                        .select_bound::<i64, Option<i64>>(
                            "SELECT project_id FROM quotations WHERE id = ?1",
                        )
                        .context("prepare project_id select")?
                        (quotation_id)
                        .context("execute project_id select")?
                        .into_iter().next()
                        .flatten();

                    conn.exec_bound::<(i64, f64, String, Option<i64>)>(
                        "INSERT INTO stock_entries
                         (product_id, quantity, unit_cost_usd, acquired_at, acquisition_type, project_id)
                         VALUES (?1, ?2, 0.0, ?3, 'project', ?4)",
                    )
                    .context("prepare stock reversal")?
                    ((pid, quantity, now, project_id))
                    .context("execute stock reversal")?;
                }
            }

            Ok(())
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
                    id             INTEGER PRIMARY KEY AUTOINCREMENT,
                    name           TEXT NOT NULL,
                    client_name    TEXT NOT NULL,
                    client_address TEXT,
                    client_attn    TEXT,
                    client_tel     TEXT,
                    description    TEXT,
                    status         TEXT NOT NULL DEFAULT 'active',
                    created_at     TEXT NOT NULL
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
