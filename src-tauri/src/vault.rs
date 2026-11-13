use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use data_encoding::BASE64;
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::{params, Connection, OpenFlags};
use zeroize::Zeroize;

use crate::database::LocalDatabase;
use crate::models::{AccountRecord, GroupRecord, TotpAlgorithm};

const KEYRING_SERVICE: &str = "dev.authapp.desktop";
const KEYRING_USER: &str = "vault-master-key";
const VAULT_BLOB_PREFIX: &[u8] = b"AUTHLY_VAULT_V1:";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

pub struct VaultDecryptResult {
    pub secret: String,
    pub re_encrypted: Option<Vec<u8>>,
}

pub trait VaultService: Send + Sync {
    fn encrypt_secret(&self, secret: &str) -> Result<Vec<u8>, String>;
    fn decrypt_secret(&self, encrypted_secret: &[u8]) -> Result<VaultDecryptResult, String>;
    fn clear_cache(&self) {}
}

pub struct KeyringAesGcmVault {
    cached_key: Mutex<Option<[u8; KEY_LEN]>>,
}

impl KeyringAesGcmVault {
    pub fn new() -> Self {
        Self {
            cached_key: Mutex::new(None),
        }
    }

    fn load_or_create_master_key(&self) -> Result<[u8; KEY_LEN], String> {
        let mut guard = self
            .cached_key
            .lock()
            .map_err(|_| "Kasa anahtarı kilidi alınamadı.".to_string())?;
        if let Some(key) = *guard {
            return Ok(key);
        }

        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|_| "İşletim sistemi anahtarlık erişimi başlatılamadı.".to_string())?;

        let key = match entry.get_password() {
            Ok(mut hex_key) => {
                let decoded = hex::decode(hex_key.trim())
                    .map_err(|_| "Anahtarlık içindeki kasa anahtarı bozuk.".to_string());
                hex_key.zeroize();
                let mut bytes = decoded?;
                if bytes.len() != KEY_LEN {
                    bytes.zeroize();
                    return Err("Anahtarlık içindeki kasa anahtarı uzunluğu geçersiz.".to_string());
                }
                let mut arr = [0u8; KEY_LEN];
                arr.copy_from_slice(&bytes);
                bytes.zeroize();
                arr
            }
            Err(keyring::Error::NoEntry) => {
                let mut arr = [0u8; KEY_LEN];
                OsRng.fill_bytes(&mut arr);
                let mut hex_key = hex::encode(arr);
                let store_res = entry.set_password(&hex_key);
                hex_key.zeroize();
                if let Err(err) = store_res {
                    arr.zeroize();
                    return Err(format!(
                        "Güvenli işletim sistemi anahtarlığı kullanılamıyor: {err}"
                    ));
                }
                arr
            }
            Err(err) => {
                return Err(format!(
                    "Güvenli işletim sistemi anahtarlığı kullanılamıyor: {err}"
                ));
            }
        };

        *guard = Some(key);
        Ok(key)
    }
}

impl VaultService for KeyringAesGcmVault {
    fn encrypt_secret(&self, secret: &str) -> Result<Vec<u8>, String> {
        let mut key_bytes = self.load_or_create_master_key()?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
        key_bytes.zeroize();

        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, secret.as_bytes())
            .map_err(|_| "Gizli anahtar şifrelenemedi.".to_string())?;

        let mut blob = Vec::with_capacity(VAULT_BLOB_PREFIX.len() + NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(VAULT_BLOB_PREFIX);
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    fn decrypt_secret(&self, encrypted_secret: &[u8]) -> Result<VaultDecryptResult, String> {
        if !encrypted_secret.starts_with(VAULT_BLOB_PREFIX) {
            return Err("Şifreli gizli anahtar biçimi tanınmıyor.".to_string());
        }
        let rest = &encrypted_secret[VAULT_BLOB_PREFIX.len()..];
        if rest.len() <= NONCE_LEN {
            return Err("Şifreli gizli anahtar verisi eksik veya bozuk.".to_string());
        }

        let (nonce_bytes, ciphertext) = rest.split_at(NONCE_LEN);
        let mut key_bytes = self.load_or_create_master_key()?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
        key_bytes.zeroize();

        let plaintext_bytes = cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|_| "Gizli anahtar çözülemedi.".to_string())?;

        let secret = String::from_utf8(plaintext_bytes)
            .map_err(|_| "Çözülen gizli anahtar geçerli UTF-8 değil.".to_string())?;

        Ok(VaultDecryptResult {
            secret,
            re_encrypted: None,
        })
    }

    fn clear_cache(&self) {
        if let Ok(mut guard) = self.cached_key.lock() {
            if let Some(ref mut key) = *guard {
                key.zeroize();
            }
            *guard = None;
        }
    }
}

pub struct InMemoryAesGcmVault {
    key: [u8; KEY_LEN],
}

impl InMemoryAesGcmVault {
    pub fn new() -> Self {
        let mut key = [0u8; KEY_LEN];
        OsRng.fill_bytes(&mut key);
        Self { key }
    }
}

impl VaultService for InMemoryAesGcmVault {
    fn encrypt_secret(&self, secret: &str) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), secret.as_bytes())
            .map_err(|_| "Gizli anahtar şifrelenemedi.".to_string())?;
        let mut blob = Vec::with_capacity(VAULT_BLOB_PREFIX.len() + NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(VAULT_BLOB_PREFIX);
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    fn decrypt_secret(&self, encrypted_secret: &[u8]) -> Result<VaultDecryptResult, String> {
        if !encrypted_secret.starts_with(VAULT_BLOB_PREFIX) {
            return Err("Şifreli gizli anahtar biçimi tanınmıyor.".to_string());
        }
        let rest = &encrypted_secret[VAULT_BLOB_PREFIX.len()..];
        if rest.len() <= NONCE_LEN {
            return Err("Şifreli gizli anahtar verisi eksik veya bozuk.".to_string());
        }
        let (nonce_bytes, ciphertext) = rest.split_at(NONCE_LEN);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let plaintext_bytes = cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|_| "Gizli anahtar çözülemedi.".to_string())?;
        let secret = String::from_utf8(plaintext_bytes)
            .map_err(|_| "Çözülen gizli anahtar geçerli UTF-8 değil.".to_string())?;
        Ok(VaultDecryptResult {
            secret,
            re_encrypted: None,
        })
    }
}

const LEGACY_V10_PREFIX: &[u8] = b"v10";

pub fn decrypt_v10_secret(
    master_key: &[u8; KEY_LEN],
    encrypted_secret: &[u8],
) -> Result<String, String> {
    if !encrypted_secret.starts_with(LEGACY_V10_PREFIX) {
        return Err("Eski şifreli gizli anahtar v10 biçiminde değil.".to_string());
    }
    let rest = &encrypted_secret[LEGACY_V10_PREFIX.len()..];
    if rest.len() <= NONCE_LEN {
        return Err("Eski şifreli gizli anahtar verisi eksik veya bozuk.".to_string());
    }

    let (nonce_bytes, ciphertext) = rest.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master_key));
    let mut plaintext_bytes = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| "Eski Electron gizli anahtarı çözülemedi.".to_string())?;

    let secret = String::from_utf8(plaintext_bytes.clone());
    plaintext_bytes.zeroize();
    secret.map_err(|_| "Çözülen eski gizli anahtar geçerli UTF-8 değil.".to_string())
}

#[cfg(windows)]
mod dpapi {
    use std::ptr;
    use zeroize::Zeroize;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptUnprotectData(
            p_data_in: *const DataBlob,
            ppsz_data_descr: *mut *mut u16,
            p_optional_entropy: *const DataBlob,
            pv_reserved: *mut core::ffi::c_void,
            p_prompt_struct: *const core::ffi::c_void,
            dw_flags: u32,
            p_data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(h_mem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    }

    pub fn unprotect(encrypted: &[u8]) -> Result<Vec<u8>, String> {
        if encrypted.is_empty() {
            return Err("DPAPI anahtar verisi boş.".to_string());
        }
        let mut input_copy = encrypted.to_vec();
        let in_blob = DataBlob {
            cb_data: u32::try_from(input_copy.len())
                .map_err(|_| "DPAPI verisi izin verilen boyutu aşıyor.".to_string())?,
            pb_data: input_copy.as_mut_ptr(),
        };
        let mut out_blob = DataBlob {
            cb_data: 0,
            pb_data: ptr::null_mut(),
        };

        // CRYPTPROTECT_UI_FORBIDDEN = 0x1 ensures silent DPAPI decryption without UI prompts.
        let ok = unsafe {
            CryptUnprotectData(
                &in_blob,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null(),
                0x1,
                &mut out_blob,
            )
        };
        input_copy.zeroize();

        if ok == 0 {
            return Err("Windows DPAPI anahtarı çözülemedi.".to_string());
        }

        let plaintext = unsafe {
            let slice = std::slice::from_raw_parts(out_blob.pb_data, out_blob.cb_data as usize);
            let out = slice.to_vec();
            if !out_blob.pb_data.is_null() {
                ptr::write_bytes(out_blob.pb_data, 0, out_blob.cb_data as usize);
                LocalFree(out_blob.pb_data.cast());
            }
            out
        };
        Ok(plaintext)
    }
}

#[cfg(windows)]
fn load_dpapi_master_key(local_state_path: &Path) -> Result<[u8; KEY_LEN], String> {
    let raw_json = std::fs::read_to_string(local_state_path)
        .map_err(|e| format!("Local State dosyası okunamadı: {e}"))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw_json)
        .map_err(|e| format!("Local State JSON çözümlenemedi: {e}"))?;

    let enc_key_b64 = parsed
        .get("os_crypt")
        .and_then(|v| v.get("encrypted_key"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Local State içinde os_crypt.encrypted_key bulunamadı.".to_string())?;

    let mut raw_key = BASE64
        .decode(enc_key_b64.trim().as_bytes())
        .map_err(|_| "Local State encrypted_key Base64 çözülemedi.".to_string())?;

    if !raw_key.starts_with(b"DPAPI") {
        raw_key.zeroize();
        return Err("Local State anahtarı DPAPI ön eki taşımıyor.".to_string());
    }

    let unprotected = dpapi::unprotect(&raw_key[5..]);
    raw_key.zeroize();
    let mut key_bytes = unprotected?;
    if key_bytes.len() != KEY_LEN {
        key_bytes.zeroize();
        return Err("Çözülen DPAPI anahtar uzunluğu geçersiz.".to_string());
    }

    let mut master_key = [0u8; KEY_LEN];
    master_key.copy_from_slice(&key_bytes);
    key_bytes.zeroize();
    Ok(master_key)
}

fn legacy_table_exists(conn: &Connection, table_name: &str) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table_name],
            |row| row.get(0),
        )
        .map_err(|e| format!("Eski veritabanı tablo kontrolü başarısız: {e}"))?;
    Ok(count > 0)
}

pub fn migrate_legacy_db_with_master_key(
    legacy_db_path: &Path,
    master_key: &[u8; KEY_LEN],
    target_db: &LocalDatabase,
    vault: &dyn VaultService,
) -> Result<usize, String> {
    if !legacy_db_path.exists() {
        return Ok(0);
    }

    let target_conn_arc = target_db.connection();
    {
        let guard = target_conn_arc
            .lock()
            .map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let existing_count: i64 = guard
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
            .map_err(|e| format!("Hedef hesap sayısı kontrol edilemedi: {e}"))?;
        if existing_count > 0 {
            return Ok(0);
        }
    }

    let legacy_conn =
        Connection::open_with_flags(legacy_db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("Eski Electron veritabanı açılamadı: {e}"))?;
    legacy_conn
        .busy_timeout(Duration::from_millis(5_000))
        .map_err(|e| format!("Eski veritabanı zaman aşımı hatası: {e}"))?;

    if !legacy_table_exists(&legacy_conn, "accounts")? {
        return Ok(0);
    }

    let mut groups = Vec::new();
    if legacy_table_exists(&legacy_conn, "groups")? {
        let mut stmt = legacy_conn
            .prepare(
                "SELECT id, name, sort_order, created_at, updated_at
                 FROM groups ORDER BY sort_order ASC, created_at ASC",
            )
            .map_err(|e| format!("Eski grup sorgusu hazırlanamadı: {e}"))?;
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
            .map_err(|e| format!("Eski gruplar okunamadı: {e}"))?;
        for row in rows {
            groups.push(row.map_err(|e| format!("Eski grup satırı okunamadı: {e}"))?);
        }
    }

    let mut raw_accounts = Vec::new();
    {
        let mut stmt = legacy_conn
            .prepare(
                "SELECT id, issuer, account_name, encrypted_secret, algorithm, digits, period,
                 favorite, group_id, sort_order, created_at, updated_at
                 FROM accounts ORDER BY favorite DESC, sort_order ASC, created_at ASC",
            )
            .map_err(|e| format!("Eski hesap sorgusu hazırlanamadı: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                let algo_str: String = row.get(4)?;
                let fav: i64 = row.get(7)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    algo_str,
                    row.get::<_, u32>(5)?,
                    row.get::<_, u32>(6)?,
                    fav == 1,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                ))
            })
            .map_err(|e| format!("Eski hesaplar okunamadı: {e}"))?;
        for row in rows {
            raw_accounts.push(row.map_err(|e| format!("Eski hesap satırı okunamadı: {e}"))?);
        }
    }

    if raw_accounts.is_empty() {
        return Ok(0);
    }

    let mut migrated_accounts = Vec::with_capacity(raw_accounts.len());
    for (
        id,
        issuer,
        account_name,
        encrypted_blob,
        algo_str,
        digits,
        period,
        favorite,
        group_id,
        sort_order,
        created_at,
        updated_at,
    ) in raw_accounts
    {
        let algorithm = TotpAlgorithm::from_str_strict(&algo_str)?;
        let re_encrypted = match decrypt_v10_secret(master_key, &encrypted_blob) {
            Ok(mut plaintext_secret) => {
                let re_encrypted_res = vault.encrypt_secret(&plaintext_secret);
                plaintext_secret.zeroize();
                re_encrypted_res?
            }
            // Keep corrupted legacy rows isolated rather than aborting migration of healthy rows.
            Err(_) => encrypted_blob,
        };

        migrated_accounts.push(AccountRecord {
            id,
            issuer,
            account_name,
            encrypted_secret: re_encrypted,
            algorithm,
            digits,
            period,
            favorite,
            group_id,
            sort_order,
            created_at,
            updated_at,
        });
    }

    let mut legacy_settings = Vec::new();
    if legacy_table_exists(&legacy_conn, "settings")? {
        let mut stmt = legacy_conn
            .prepare("SELECT key, value, updated_at FROM settings")
            .map_err(|e| format!("Eski ayar sorgusu hazırlanamadı: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|e| format!("Eski ayarlar okunamadı: {e}"))?;
        for row in rows {
            legacy_settings.push(row.map_err(|e| format!("Eski ayar satırı okunamadı: {e}"))?);
        }
    }

    drop(legacy_conn);

    let mut guard = target_conn_arc
        .lock()
        .map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
    let tx = guard
        .transaction()
        .map_err(|e| format!("Aktarım işlemi başlatılamadı: {e}"))?;

    let current_count: i64 = tx
        .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
        .map_err(|e| format!("Hedef hesap sayısı tekrar kontrol edilemedi: {e}"))?;
    if current_count > 0 {
        return Ok(0);
    }

    for group in &groups {
        tx.execute(
            "INSERT OR IGNORE INTO groups (id, name, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                group.id,
                group.name,
                group.sort_order,
                group.created_at,
                group.updated_at
            ],
        )
        .map_err(|e| format!("Grup aktarılamadı: {e}"))?;
    }

    for account in &migrated_accounts {
        tx.execute(
            "INSERT INTO accounts (
                id, issuer, account_name, encrypted_secret, algorithm, digits, period,
                favorite, group_id, sort_order, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                account.id,
                account.issuer,
                account.account_name,
                account.encrypted_secret,
                account.algorithm.as_str(),
                account.digits,
                account.period,
                if account.favorite { 1i64 } else { 0i64 },
                account.group_id,
                account.sort_order,
                account.created_at,
                account.updated_at,
            ],
        )
        .map_err(|e| format!("Hesap aktarılamadı: {e}"))?;
    }

    for (key, value, updated_at) in &legacy_settings {
        tx.execute(
            "INSERT OR IGNORE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, updated_at],
        )
        .map_err(|e| format!("Ayar aktarılamadı: {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("Aktarım işlemi onaylanamadı: {e}"))?;

    Ok(migrated_accounts.len())
}

#[cfg(windows)]
pub fn migrate_legacy_electron_db(
    legacy_dir: &Path,
    target_db: &LocalDatabase,
    vault: &dyn VaultService,
) -> Result<usize, String> {
    let legacy_db_path = legacy_dir.join("authapp.db");
    let local_state_path = legacy_dir.join("Local State");
    if !legacy_db_path.exists() || !local_state_path.exists() {
        return Ok(0);
    }

    {
        let conn_arc = target_db.connection();
        let guard = conn_arc
            .lock()
            .map_err(|_| "Veritabanı kilidi alınamadı.".to_string())?;
        let existing_count: i64 = guard
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
            .map_err(|e| format!("Hedef hesap sayısı kontrol edilemedi: {e}"))?;
        if existing_count > 0 {
            return Ok(0);
        }
    }

    let mut master_key = load_dpapi_master_key(&local_state_path)?;
    let migrated = migrate_legacy_db_with_master_key(&legacy_db_path, &master_key, target_db, vault);
    master_key.zeroize();
    migrated
}

#[cfg(not(windows))]
pub fn migrate_legacy_electron_db(
    _legacy_dir: &Path,
    _target_db: &LocalDatabase,
    _vault: &dyn VaultService,
) -> Result<usize, String> {
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::AccountRepository;

    fn encrypt_v10_fixture(master_key: &[u8; KEY_LEN], secret: &str) -> Vec<u8> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(master_key));
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), secret.as_bytes())
            .unwrap();
        let mut blob = Vec::with_capacity(3 + NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(LEGACY_V10_PREFIX);
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);
        blob
    }

    #[test]
    fn encrypts_and_decrypts_secrets_with_random_nonces() {
        let vault = InMemoryAesGcmVault::new();
        let secret = "JBSWY3DPEHPK3PXP";
        let blob1 = vault.encrypt_secret(secret).unwrap();
        let blob2 = vault.encrypt_secret(secret).unwrap();
        assert_ne!(blob1, blob2);
        assert!(!String::from_utf8_lossy(&blob1).contains(secret));
        let dec = vault.decrypt_secret(&blob1).unwrap();
        assert_eq!(dec.secret, secret);
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let vault = InMemoryAesGcmVault::new();
        let mut blob = vault.encrypt_secret("JBSWY3DPEHPK3PXP").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xff;
        assert!(vault.decrypt_secret(&blob).is_err());
    }

    #[test]
    fn migrates_legacy_v10_accounts_groups_and_settings_once() {
        let mut legacy_key = [0u8; KEY_LEN];
        OsRng.fill_bytes(&mut legacy_key);

        let temp_dir = std::env::temp_dir().join(format!("authly_legacy_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let legacy_db_path = temp_dir.join("legacy.db");

        {
            let legacy_db = LocalDatabase::open(&legacy_db_path).unwrap();
            let conn_arc = legacy_db.connection();
            let guard = conn_arc.lock().unwrap();
            let group_id = "11111111-1111-4111-8111-111111111111";
            let account_id = "22222222-2222-4222-8222-222222222222";
            guard
                .execute(
                    "INSERT INTO groups (id, name, sort_order, created_at, updated_at)
                     VALUES (?1, 'Work', 0, 10, 10)",
                    params![group_id],
                )
                .unwrap();
            let v10_blob = encrypt_v10_fixture(&legacy_key, "JBSWY3DPEHPK3PXP");
            guard
                .execute(
                    "INSERT INTO accounts (
                        id, issuer, account_name, encrypted_secret, algorithm, digits, period,
                        favorite, group_id, sort_order, created_at, updated_at
                    ) VALUES (?1, 'GitHub', 'user@example.com', ?2, 'SHA1', 6, 30, 1, ?3, 0, 100, 100)",
                    params![account_id, v10_blob, group_id],
                )
                .unwrap();
            guard
                .execute(
                    "INSERT INTO settings (key, value, updated_at) VALUES ('language', 'tr', 10)",
                    [],
                )
                .unwrap();
        }

        let target_db = LocalDatabase::open_in_memory().unwrap();
        let vault = InMemoryAesGcmVault::new();

        let count =
            migrate_legacy_db_with_master_key(&legacy_db_path, &legacy_key, &target_db, &vault)
                .unwrap();
        assert_eq!(count, 1);

        let migrated_accounts = AccountRepository::new(target_db.connection()).list().unwrap();
        assert_eq!(migrated_accounts.len(), 1);
        assert_eq!(migrated_accounts[0].issuer, "GitHub");
        assert!(migrated_accounts[0]
            .encrypted_secret
            .starts_with(VAULT_BLOB_PREFIX));
        let decrypted = vault
            .decrypt_secret(&migrated_accounts[0].encrypted_secret)
            .unwrap();
        assert_eq!(decrypted.secret, "JBSWY3DPEHPK3PXP");

        // Second run is a no-op once target database is non-empty.
        let second_count =
            migrate_legacy_db_with_master_key(&legacy_db_path, &legacy_key, &target_db, &vault)
                .unwrap();
        assert_eq!(second_count, 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn migrates_healthy_accounts_when_another_row_has_corrupted_v10_ciphertext() {
        let mut legacy_key = [0u8; KEY_LEN];
        OsRng.fill_bytes(&mut legacy_key);

        let temp_dir =
            std::env::temp_dir().join(format!("authly_legacy_corrupt_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let legacy_db_path = temp_dir.join("legacy.db");

        let healthy_id = "33333333-3333-4333-8333-333333333333";
        let corrupted_id = "44444444-4444-4444-8444-444444444444";
        let mut corrupted_blob = encrypt_v10_fixture(&legacy_key, "MFRGGZDFMZTWQ2LK");
        let last_idx = corrupted_blob.len() - 1;
        corrupted_blob[last_idx] ^= 0xff;

        {
            let legacy_db = LocalDatabase::open(&legacy_db_path).unwrap();
            let conn_arc = legacy_db.connection();
            let guard = conn_arc.lock().unwrap();
            let healthy_blob = encrypt_v10_fixture(&legacy_key, "JBSWY3DPEHPK3PXP");

            guard
                .execute(
                    "INSERT INTO accounts (
                        id, issuer, account_name, encrypted_secret, algorithm, digits, period,
                        favorite, group_id, sort_order, created_at, updated_at
                    ) VALUES (?1, 'GitHub', 'healthy@example.com', ?2, 'SHA1', 6, 30, 0, NULL, 0, 100, 100)",
                    params![healthy_id, healthy_blob],
                )
                .unwrap();
            guard
                .execute(
                    "INSERT INTO accounts (
                        id, issuer, account_name, encrypted_secret, algorithm, digits, period,
                        favorite, group_id, sort_order, created_at, updated_at
                    ) VALUES (?1, 'Corrupt', 'corrupt@example.com', ?2, 'SHA1', 6, 30, 0, NULL, 1, 100, 100)",
                    params![corrupted_id, corrupted_blob],
                )
                .unwrap();
        }

        let target_db = LocalDatabase::open_in_memory().unwrap();
        let vault = InMemoryAesGcmVault::new();

        let count =
            migrate_legacy_db_with_master_key(&legacy_db_path, &legacy_key, &target_db, &vault)
                .unwrap();
        assert_eq!(count, 2);

        let migrated_accounts = AccountRepository::new(target_db.connection()).list().unwrap();
        assert_eq!(migrated_accounts.len(), 2);

        let healthy = migrated_accounts
            .iter()
            .find(|acc| acc.id == healthy_id)
            .expect("healthy account should be migrated");
        assert!(healthy.encrypted_secret.starts_with(VAULT_BLOB_PREFIX));
        assert_eq!(
            vault.decrypt_secret(&healthy.encrypted_secret).unwrap().secret,
            "JBSWY3DPEHPK3PXP"
        );

        let corrupted = migrated_accounts
            .iter()
            .find(|acc| acc.id == corrupted_id)
            .expect("corrupted account row should remain isolated in target db");
        assert_eq!(corrupted.encrypted_secret, corrupted_blob);
        assert!(vault.decrypt_secret(&corrupted.encrypted_secret).is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn clear_cache_zeroizes_and_clears_cached_master_key() {
        let vault = KeyringAesGcmVault::new();
        {
            let mut guard = vault.cached_key.lock().unwrap();
            *guard = Some([0xAB; KEY_LEN]);
        }
        assert!(vault.cached_key.lock().unwrap().is_some());

        vault.clear_cache();

        assert!(vault.cached_key.lock().unwrap().is_none());
    }
}
