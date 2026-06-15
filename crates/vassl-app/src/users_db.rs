use anyhow::Context as _;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlez::domain::Domain;
use vassl_db::SharedDomain;

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub can_inventory: bool,
    pub can_pricebook: bool,
    pub can_quotations: bool,
    pub allow_delete: bool,
    pub allow_price_edit: bool,
    #[allow(dead_code)]
    pub must_change_password: bool,
    pub is_active: bool,
}

pub struct UsersDb(pub sqlez::thread_safe_connection::ThreadSafeConnection);

impl Domain for UsersDb {
    const NAME: &'static str = "users";
    const MIGRATIONS: &'static [&'static str] = &["CREATE TABLE IF NOT EXISTS users (
            id                    INTEGER PRIMARY KEY AUTOINCREMENT,
            username              TEXT UNIQUE NOT NULL,
            password_hash         TEXT NOT NULL,
            is_admin              INTEGER NOT NULL DEFAULT 0,
            can_inventory         INTEGER NOT NULL DEFAULT 0,
            can_pricebook         INTEGER NOT NULL DEFAULT 0,
            can_quotations        INTEGER NOT NULL DEFAULT 0,
            allow_delete          INTEGER NOT NULL DEFAULT 0,
            allow_price_edit      INTEGER NOT NULL DEFAULT 0,
            must_change_password  INTEGER NOT NULL DEFAULT 0,
            is_active             INTEGER NOT NULL DEFAULT 1,
            created_at            TEXT NOT NULL
        )"];
    fn should_allow_migration_change(_: usize, _: &str, _: &str) -> bool {
        true
    }
}

vassl_db::static_connection!(UsersDb, [SharedDomain]);

pub fn hash_password(plain: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("password hash failed: {e}"))
}

pub fn verify_password(plain: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

impl UsersDb {
    pub fn user_count(&self) -> anyhow::Result<i64> {
        let rows = self
            .select::<i64>("SELECT COUNT(*) FROM users")
            .context("prepare user_count")?()
        .context("execute user_count")?;
        Ok(rows.into_iter().next().unwrap_or(0))
    }

    pub fn list_users(&self) -> anyhow::Result<Vec<AuthUser>> {
        self.select::<(i64, String, bool, bool, bool, bool, bool, bool, bool, bool)>(
            "SELECT id, username, is_admin, can_inventory, can_pricebook, can_quotations,
                    allow_delete, allow_price_edit, must_change_password, is_active
             FROM users ORDER BY id",
        )
        .context("prepare list_users")?()
        .context("execute list_users")
        .map(|rows| {
            rows.into_iter()
                .map(
                    |(id, username, is_admin, ci, cp, cq, ad, ap, mcp, active)| AuthUser {
                        id,
                        username,
                        is_admin,
                        can_inventory: ci,
                        can_pricebook: cp,
                        can_quotations: cq,
                        allow_delete: ad,
                        allow_price_edit: ap,
                        must_change_password: mcp,
                        is_active: active,
                    },
                )
                .collect()
        })
    }

    pub async fn verify_credentials(
        &self,
        username: String,
        password: String,
    ) -> anyhow::Result<Option<AuthUser>> {
        self.write(move |conn| {
            let row = conn.select_row_bound::<&str, (i64, String, String, bool, bool, bool, bool, bool, bool, bool, bool)>(
                "SELECT id, username, password_hash, is_admin, can_inventory, can_pricebook, can_quotations,
                        allow_delete, allow_price_edit, must_change_password, is_active
                 FROM users WHERE username = (?) AND is_active = 1",
            )
            .context("prepare verify_credentials")?
            (username.as_str())
            .context("execute verify_credentials")?;

            let Some((id, uname, hash, is_admin, ci, cp, cq, ad, ap, mcp, active)) = row else {
                return Ok(None);
            };

            if !verify_password(&password, &hash) {
                return Ok(None);
            }

            Ok(Some(AuthUser {
                id, username: uname, is_admin,
                can_inventory: ci, can_pricebook: cp, can_quotations: cq,
                allow_delete: ad, allow_price_edit: ap,
                must_change_password: mcp, is_active: active,
            }))
        }).await
    }

    pub async fn insert_admin(&self, username: String, password: &str) -> anyhow::Result<i64> {
        let hash = hash_password(password)?;
        let now = chrono::Utc::now().to_rfc3339();
        self.write(move |conn| {
            conn.exec_bound::<(String, String, String)>(
                "INSERT INTO users
                 (username, password_hash, is_admin, can_inventory, can_pricebook, can_quotations,
                  allow_delete, allow_price_edit, created_at)
                 VALUES (?, ?, 1, 1, 1, 1, 1, 1, ?)",
            )
            .context("prepare insert_admin")?((username, hash, now))
            .context("execute insert_admin")?;
            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare rowid")?()
            .context("execute rowid")?
            .context("rowid was None")
        })
        .await
    }

    pub async fn insert_user(
        &self,
        username: String,
        password: &str,
        can_inventory: bool,
        can_pricebook: bool,
        can_quotations: bool,
        allow_delete: bool,
        allow_price_edit: bool,
    ) -> anyhow::Result<i64> {
        let hash = hash_password(password)?;
        let now = chrono::Utc::now().to_rfc3339();
        self.write(move |conn| {
            conn.exec_bound::<(String, String, bool, bool, bool, bool, bool, String)>(
                "INSERT INTO users
                 (username, password_hash, can_inventory, can_pricebook, can_quotations,
                  allow_delete, allow_price_edit, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .context("prepare insert_user")?((
                username,
                hash,
                can_inventory,
                can_pricebook,
                can_quotations,
                allow_delete,
                allow_price_edit,
                now,
            ))
            .context("execute insert_user")?;
            conn.select_row::<i64>("SELECT last_insert_rowid()")
                .context("prepare rowid")?()
            .context("execute rowid")?
            .context("rowid was None")
        })
        .await
    }

    pub async fn update_user_permissions(
        &self,
        id: i64,
        can_inventory: bool,
        can_pricebook: bool,
        can_quotations: bool,
        allow_delete: bool,
        allow_price_edit: bool,
    ) -> anyhow::Result<()> {
        self.write(move |conn| {
            conn.exec_bound::<(bool, bool, bool, bool, bool, i64)>(
                "UPDATE users SET can_inventory=?, can_pricebook=?, can_quotations=?,
                 allow_delete=?, allow_price_edit=? WHERE id=? AND is_admin=0",
            )
            .context("prepare update_permissions")?((
                can_inventory,
                can_pricebook,
                can_quotations,
                allow_delete,
                allow_price_edit,
                id,
            ))
            .context("execute update_permissions")
        })
        .await
    }

    pub async fn reset_password(&self, id: i64, new_password: &str) -> anyhow::Result<()> {
        let hash = hash_password(new_password)?;
        self.write(move |conn| {
            conn.exec_bound::<(String, i64)>(
                "UPDATE users SET password_hash=?, must_change_password=1 WHERE id=?",
            )
            .context("prepare reset_password")?((hash, id))
            .context("execute reset_password")
        })
        .await
    }

    pub async fn change_password(
        &self,
        id: i64,
        old_password: String,
        new_password: &str,
    ) -> anyhow::Result<()> {
        let stored_rows = self
            .select_bound::<i64, String>("SELECT password_hash FROM users WHERE id=?1")
            .context("prepare read hash")?(id)
        .context("execute read hash")?;
        let stored = stored_rows.into_iter().next().context("user not found")?;
        if !verify_password(&old_password, &stored) {
            anyhow::bail!("Current password is incorrect.");
        }
        let new_hash = hash_password(new_password)?;
        self.write(move |conn| {
            conn.exec_bound::<(String, i64)>(
                "UPDATE users SET password_hash=?, must_change_password=0 WHERE id=?",
            )
            .context("prepare change_password")?((new_hash, id))
            .context("execute change_password")
        })
        .await
    }

    pub async fn deactivate_user(&self, id: i64) -> anyhow::Result<()> {
        self.write(move |conn| {
            conn.exec_bound::<i64>("UPDATE users SET is_active=0 WHERE id=? AND is_admin=0")
                .context("prepare deactivate")?(id)
            .context("execute deactivate")
        })
        .await
    }

    pub async fn reactivate_user(&self, id: i64) -> anyhow::Result<()> {
        self.write(move |conn| {
            conn.exec_bound::<i64>("UPDATE users SET is_active=1 WHERE id=? AND is_admin=0")
                .context("prepare reactivate")?(id)
            .context("execute reactivate")
        })
        .await
    }

    pub async fn log_auth_event(
        &self,
        user_id: i64,
        action: &str,
        username: &str,
    ) -> anyhow::Result<()> {
        let action = action.to_string();
        let username = username.to_string();
        self.write(move |conn| {
            vassl_db::shared::log_audit(conn, "users", user_id, &action, &username, None, None)?;
            Ok(())
        })
        .await
    }
}
