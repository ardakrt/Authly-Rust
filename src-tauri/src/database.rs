use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{AccountRecord, GroupRecord, TotpAlgorithm};

const MIGRATION_1_SQL: &str = r#"
  CREATE TABLE groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
  ) STRICT;

  CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    issuer TEXT NOT NULL,
    account_name TEXT NOT NULL,
    encrypted_secret BLOB NOT NULL,
    algorithm TEXT NOT NULL CHECK (algorithm IN ('SHA1', 'SHA256', 'SHA512')),
    digits INTEGER NOT NULL CHECK (digits BETWEEN 6 AND 10),
    period INTEGER NOT NULL CHECK (period BETWEEN 5 AND 300),
    favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1)),
    group_id TEXT REFERENCES groups(id) ON DELETE SET NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
  ) STRICT;

  CREATE INDEX accounts_order_idx ON accounts(favorite DESC, sort_order ASC, created_at ASC);
  CREATE INDEX accounts_group_idx ON accounts(group_id);

  CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
  ) STRICT;
"#;

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Clone)]
pub struct LocalDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl LocalDatabase {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Veritabanı dizini oluşturulamadı: {e}"))?;
        }
        let conn = Connection::open(path).map_err(|e| format!("Veritabanı açılamadı: {e}"))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, String> {
        let conn =
            Connection::open_in_memory().map_err(|e| format!("Bellek veritabanı açılamadı: {e}"))?;
        Self::init(conn)
    }

    fn init(mut conn: Connection) -> Result<Self, String> {
        conn.busy_timeout(Duration::from_millis(5_000))
            .map_err(|e| format!("Veritabanı zaman aşımı ayarlanamadı: {e}"))?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| format!("foreign_keys pragma hatası: {e}"))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| format!("journal_mode pragma hatası: {e}"))?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(|e| format!("synchronous pragma hatası: {e}"))?;

        run_migrations(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }
}

fn run_migrations(conn: &mut Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            id INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        ) STRICT;",
    )
    .map_err(|e| format!("schema_migrations oluşturulamadı: {e}"))?;

    let applied: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM schema_migrations WHERE id = ?1",
            params![1i64],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Migrasyon sorgusu başarısız: {e}"))?;

    if applied.is_none() {
        let tx = conn
            .transaction()
            .map_err(|e| format!("Migrasyon işlemi başlatılamadı: {e}"))?;
        tx.execute_batch(MIGRATION_1_SQL)
            .map_err(|e| format!("Migrasyon uygulanamadı: {e}"))?;
        tx.execute(
            "INSERT INTO schema_migrations (id, applied_at) VALUES (?1, ?2)",
            params![1i64, now_millis()],
        )
        .map_err(|e| format!("Migrasyon kaydı yazılamadı: {e}"))?;
        tx.commit()
            .map_err(|e| format!("Migrasyon onaylanamadı: {e}"))?;
    }

    Ok(())
}

#[derive(Clone)]
pub struct AccountRepository {
    conn: Arc<Mutex<Connection>>,
}

impl AccountRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn list(&self) -> Result<Vec<AccountRecord>, String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let mut stmt = guard
            .prepare(
                "SELECT id, issuer, account_name, encrypted_secret, algorithm, digits, period,
                 favorite, group_id, sort_order, created_at, updated_at
                 FROM accounts ORDER BY favorite DESC, sort_order, created_at",
            )
            .map_err(|e| format!("Hesap sorgusu hazırlanamadı: {e}"))?;
        let rows = stmt
            .query_map([], map_account_row)
            .map_err(|e| format!("Hesaplar listelenemedi: {e}"))?;
        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row.map_err(|e| format!("Hesap satırı okunamadı: {e}"))?);
        }
        Ok(accounts)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<AccountRecord>, String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        guard
            .query_row(
                "SELECT id, issuer, account_name, encrypted_secret, algorithm, digits, period,
                 favorite, group_id, sort_order, created_at, updated_at
                 FROM accounts WHERE id = ?1",
                params![id],
                map_account_row,
            )
            .optional()
            .map_err(|e| format!("Hesap okunamadı: {e}"))
    }

    pub fn create(&self, record: &AccountRecord) -> Result<(), String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        insert_account(&guard, record)
    }

    pub fn create_many(&self, records: &[AccountRecord]) -> Result<(), String> {
        let mut guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let tx = guard
            .transaction()
            .map_err(|e| format!("Toplu hesap işlemi başlatılamadı: {e}"))?;
        for record in records {
            insert_account(&tx, record)?;
        }
        tx.commit()
            .map_err(|e| format!("Toplu hesap işlemi onaylanamadı: {e}"))?;
        Ok(())
    }

    pub fn restore_groups_and_accounts(
        &self,
        groups: &[GroupRecord],
        accounts: &[AccountRecord],
    ) -> Result<(), String> {
        let mut guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let tx = guard
            .transaction()
            .map_err(|e| format!("Yedek geri yükleme işlemi başlatılamadı: {e}"))?;
        for group in groups {
            insert_group(&tx, group)?;
        }
        for record in accounts {
            insert_account(&tx, record)?;
        }
        tx.commit()
            .map_err(|e| format!("Yedek geri yükleme işlemi onaylanamadı: {e}"))?;
        Ok(())
    }

    pub fn update(&self, record: &AccountRecord) -> Result<(), String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let changes = guard
            .execute(
                "UPDATE accounts SET issuer = ?1, account_name = ?2, encrypted_secret = ?3,
                 algorithm = ?4, digits = ?5, period = ?6, favorite = ?7, group_id = ?8, updated_at = ?9
                 WHERE id = ?10",
                params![
                    record.issuer,
                    record.account_name,
                    record.encrypted_secret,
                    record.algorithm.as_str(),
                    record.digits,
                    record.period,
                    if record.favorite { 1i64 } else { 0i64 },
                    record.group_id,
                    record.updated_at,
                    record.id,
                ],
            )
            .map_err(|e| format!("Hesap güncellenemedi: {e}"))?;
        if changes != 1 {
            return Err("Account not found.".to_string());
        }
        Ok(())
    }

    pub fn update_encrypted_secret(&self, id: &str, encrypted_secret: &[u8]) -> Result<(), String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let changes = guard
            .execute(
                "UPDATE accounts SET encrypted_secret = ?1 WHERE id = ?2",
                params![encrypted_secret, id],
            )
            .map_err(|e| format!("Şifreli anahtar güncellenemedi: {e}"))?;
        if changes != 1 {
            return Err("Account not found.".to_string());
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let changes = guard
            .execute("DELETE FROM accounts WHERE id = ?1", params![id])
            .map_err(|e| format!("Hesap silinemedi: {e}"))?;
        Ok(changes == 1)
    }
}

fn insert_account(conn: &Connection, record: &AccountRecord) -> Result<(), String> {
    conn.execute(
        "INSERT INTO accounts (
            id, issuer, account_name, encrypted_secret, algorithm, digits, period,
            favorite, group_id, sort_order, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            record.id,
            record.issuer,
            record.account_name,
            record.encrypted_secret,
            record.algorithm.as_str(),
            record.digits,
            record.period,
            if record.favorite { 1i64 } else { 0i64 },
            record.group_id,
            record.sort_order,
            record.created_at,
            record.updated_at,
        ],
    )
    .map_err(|e| format!("Hesap veritabanına yazılamadı: {e}"))?;
    Ok(())
}

fn map_account_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AccountRecord> {
    let algo_str: String = row.get(4)?;
    let algorithm = TotpAlgorithm::from_str_strict(&algo_str).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid algorithm")),
        )
    })?;
    let fav: i64 = row.get(7)?;
    Ok(AccountRecord {
        id: row.get(0)?,
        issuer: row.get(1)?,
        account_name: row.get(2)?,
        encrypted_secret: row.get(3)?,
        algorithm,
        digits: row.get(5)?,
        period: row.get(6)?,
        favorite: fav == 1,
        group_id: row.get(8)?,
        sort_order: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

#[derive(Clone)]
pub struct GroupRepository {
    conn: Arc<Mutex<Connection>>,
}

impl GroupRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn list(&self) -> Result<Vec<GroupRecord>, String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let mut stmt = guard
            .prepare(
                "SELECT id, name, sort_order, created_at, updated_at
                 FROM groups ORDER BY sort_order, created_at",
            )
            .map_err(|e| format!("Grup sorgusu hazırlanamadı: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(GroupRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sort_order: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(|e| format!("Gruplar listelenemedi: {e}"))?;
        let mut groups = Vec::new();
        for row in rows {
            groups.push(row.map_err(|e| format!("Grup satırı okunamadı: {e}"))?);
        }
        Ok(groups)
    }

    pub fn create(&self, group: &GroupRecord) -> Result<(), String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        insert_group(&guard, group)
    }

    pub fn update(&self, group: &GroupRecord) -> Result<(), String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let changes = guard
            .execute(
                "UPDATE groups SET name = ?1, sort_order = ?2, updated_at = ?3 WHERE id = ?4",
                params![group.name, group.sort_order, group.updated_at, group.id],
            )
            .map_err(|e| format!("Grup güncellenemedi: {e}"))?;
        if changes != 1 {
            return Err("Group not found.".to_string());
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let changes = guard
            .execute("DELETE FROM groups WHERE id = ?1", params![id])
            .map_err(|e| format!("Grup silinemedi: {e}"))?;
        Ok(changes == 1)
    }
}

fn insert_group(conn: &Connection, group: &GroupRecord) -> Result<(), String> {
    conn.execute(
        "INSERT INTO groups (id, name, sort_order, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![group.id, group.name, group.sort_order, group.created_at, group.updated_at],
    )
    .map_err(|e| format!("Grup veritabanına yazılamadı: {e}"))?;
    Ok(())
}

#[derive(Clone)]
pub struct SettingsRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SettingsRepository {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        guard
            .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| format!("Ayar okunamadı: {e}"))
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        guard
            .execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value, now_millis()],
            )
            .map_err(|e| format!("Ayar kaydedilemedi: {e}"))?;
        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<bool, String> {
        let guard = self.conn.lock().map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let changes = guard
            .execute("DELETE FROM settings WHERE key = ?1", params![key])
            .map_err(|e| format!("Ayar silinemedi: {e}"))?;
        Ok(changes == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_group_and_settings_crud() {
        let db = LocalDatabase::open_in_memory().expect("db");
        let groups = GroupRepository::new(db.connection());
        let settings = SettingsRepository::new(db.connection());

        let group = GroupRecord {
            id: "work".to_string(),
            name: "Work".to_string(),
            sort_order: 0,
            created_at: 1,
            updated_at: 1,
        };
        groups.create(&group).unwrap();
        assert_eq!(groups.list().unwrap(), vec![group.clone()]);

        let updated = GroupRecord {
            name: "Personal".to_string(),
            updated_at: 2,
            ..group
        };
        groups.update(&updated).unwrap();
        assert_eq!(groups.list().unwrap()[0].name, "Personal");
        assert!(groups.delete("work").unwrap());
        assert!(groups.list().unwrap().is_empty());

        assert_eq!(settings.get("theme").unwrap(), None);
        settings.set("theme", "dark").unwrap();
        settings.set("theme", "light").unwrap();
        assert_eq!(settings.get("theme").unwrap(), Some("light".to_string()));
        assert!(settings.delete("theme").unwrap());
        assert_eq!(settings.get("theme").unwrap(), None);
    }

    #[test]
    fn runs_migrations_idempotently() {
        let db = LocalDatabase::open_in_memory().expect("db");
        let conn_arc = db.connection();
        {
            let mut guard = conn_arc.lock().unwrap();
            run_migrations(&mut guard).unwrap();
            let count: i64 = guard
                .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 1);
        }
    }
}
