use std::fs;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use pbkdf2::pbkdf2_hmac;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroize;

use crate::account_service::{AccountService, MAX_BACKUP_ACCOUNTS};
use crate::database::now_millis;
use crate::models::{
    BackupAccountItem, BackupCipher, BackupFileEnvelope, BackupKdf, BackupPayload,
    ExportBackupResult, ImportBackupResult,
};

pub const MIN_BACKUP_PASSWORD_LENGTH: usize = 12;
pub const MAX_BACKUP_PASSWORD_LENGTH: usize = 128;
pub const MIN_BACKUP_KDF_ITERATIONS: u32 = 100_000;
pub const CURRENT_BACKUP_KDF_ITERATIONS: u32 = 600_000;
pub const MAX_BACKUP_KDF_ITERATIONS: u32 = 2_000_000;
pub const MAX_BACKUP_FILE_BYTES: usize = 20 * 1024 * 1024;
const GCM_TAG_LEN: usize = 16;

#[derive(Clone)]
pub struct BackupService {
    account_service: AccountService,
}

impl BackupService {
    pub fn new(account_service: AccountService) -> Self {
        Self { account_service }
    }

    pub fn export_backup<F>(
        &self,
        password: &str,
        assert_access: F,
    ) -> Result<ExportBackupResult, String>
    where
        F: Fn() -> Result<(), String>,
    {
        validate_export_password(password)?;
        assert_access()?;

        let default_name = format!(
            "authapp-backup-{}.authapp",
            chrono_like_date_utc(now_millis())
        );
        let picked = rfd::FileDialog::new()
            .set_title("Şifreli Yedek Kaydet")
            .set_file_name(&default_name)
            .add_filter("Authapp Backup (*.authapp)", &["authapp"])
            .save_file();

        assert_access()?;

        let Some(path) = picked else {
            return Ok(ExportBackupResult {
                success: false,
                file_path: None,
                exported_count: 0,
            });
        };

        self.export_backup_to_path(password, &path, assert_access)
    }

    pub fn export_backup_to_path<F>(
        &self,
        password: &str,
        path: &Path,
        assert_access: F,
    ) -> Result<ExportBackupResult, String>
    where
        F: Fn() -> Result<(), String>,
    {
        validate_export_password(password)?;
        assert_access()?;

        let accounts = self.account_service.list()?;
        let groups = self.account_service.list_groups()?;
        if accounts.len() > MAX_BACKUP_ACCOUNTS {
            return Err("Hesap sayısı yedek sınırını aşıyor.".to_string());
        }

        let mut export_items = Vec::with_capacity(accounts.len());
        for account in accounts {
            let (secret, _) = self.account_service.get_totp_secret(&account.id)?;
            export_items.push(BackupAccountItem {
                issuer: account.issuer,
                account_name: account.account_name,
                secret,
                algorithm: account.algorithm,
                digits: account.digits,
                period: account.period,
                favorite: account.favorite,
                group_id: account.group_id,
            });
        }

        let exported_count = export_items.len();
        let mut payload = BackupPayload {
            version: 2,
            groups: Some(groups),
            exported_at: now_millis(),
            accounts: export_items,
        };

        let mut payload_json = serde_json::to_string(&payload)
            .map_err(|_| "Yedek verisi serileştirilemedi.".to_string())?;
        for item in &mut payload.accounts {
            item.secret.zeroize();
        }

        let mut salt = [0u8; 16];
        let mut iv = [0u8; 12];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut iv);

        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha256>(
            password.as_bytes(),
            &salt,
            CURRENT_BACKUP_KDF_ITERATIONS,
            &mut key,
        );

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        key.zeroize();

        let ct_res = cipher
            .encrypt(Nonce::from_slice(&iv), payload_json.as_bytes())
            .map_err(|_| "Yedek şifrelenemedi.".to_string());
        payload_json.zeroize();
        let ct_and_tag = ct_res?;

        if ct_and_tag.len() < GCM_TAG_LEN {
            return Err("Yedek şifreleme çıktısı geçersiz.".to_string());
        }
        let split_at = ct_and_tag.len() - GCM_TAG_LEN;
        let encrypted_bytes = &ct_and_tag[..split_at];
        let auth_tag_bytes = &ct_and_tag[split_at..];

        let envelope = BackupFileEnvelope {
            format: "authapp-backup-v1".to_string(),
            kdf: BackupKdf {
                algorithm: "pbkdf2".to_string(),
                iterations: CURRENT_BACKUP_KDF_ITERATIONS,
                hash: "sha256".to_string(),
                salt: hex::encode(salt),
            },
            cipher: BackupCipher {
                algorithm: "aes-256-gcm".to_string(),
                iv: hex::encode(iv),
                auth_tag: hex::encode(auth_tag_bytes),
            },
            encrypted_data: hex::encode(encrypted_bytes),
        };

        assert_access()?;
        let file_content = serde_json::to_string_pretty(&envelope)
            .map_err(|_| "Yedek dosyası oluşturulamadı.".to_string())?;
        if file_content.len() > MAX_BACKUP_FILE_BYTES {
            return Err("Yedek dosyası izin verilen boyut sınırını aşıyor.".to_string());
        }

        fs::write(path, file_content).map_err(|e| format!("Yedek dosyası yazılamadı: {e}"))?;

        Ok(ExportBackupResult {
            success: true,
            file_path: Some(path.to_string_lossy().into_owned()),
            exported_count,
        })
    }

    pub fn import_backup<F>(
        &self,
        password: &str,
        assert_access: F,
    ) -> Result<ImportBackupResult, String>
    where
        F: Fn() -> Result<(), String>,
    {
        validate_import_password(password)?;
        assert_access()?;

        let picked = rfd::FileDialog::new()
            .set_title("Yedek Dosyası Seç")
            .add_filter("Authapp Backup (*.authapp, *.json)", &["authapp", "json"])
            .pick_file();

        assert_access()?;

        let Some(path) = picked else {
            return Ok(ImportBackupResult {
                success: false,
                imported_count: 0,
                skipped_count: 0,
                ungrouped_count: 0,
            });
        };

        self.import_backup_from_path(password, &path, assert_access)
    }

    pub fn import_backup_from_path<F>(
        &self,
        password: &str,
        path: &Path,
        assert_access: F,
    ) -> Result<ImportBackupResult, String>
    where
        F: Fn() -> Result<(), String>,
    {
        validate_import_password(password)?;
        assert_access()?;

        let metadata =
            fs::metadata(path).map_err(|_| "Yedek dosyası okunamadı.".to_string())?;
        if !metadata.is_file() {
            return Err("Geçerli bir yedek dosyası seçin.".to_string());
        }
        if metadata.len() as usize > MAX_BACKUP_FILE_BYTES {
            return Err("Yedek dosyası izin verilen boyut sınırını aşıyor.".to_string());
        }

        let file_content =
            fs::read_to_string(path).map_err(|_| "Yedek dosyası okunamadı.".to_string())?;
        if file_content.len() > MAX_BACKUP_FILE_BYTES {
            return Err("Yedek dosyası izin verilen boyut sınırını aşıyor.".to_string());
        }

        self.import_backup_from_str(password, &file_content, assert_access)
    }

    pub fn import_backup_from_str<F>(
        &self,
        password: &str,
        file_content: &str,
        assert_access: F,
    ) -> Result<ImportBackupResult, String>
    where
        F: Fn() -> Result<(), String>,
    {
        validate_import_password(password)?;
        assert_access()?;

        let envelope: BackupFileEnvelope = serde_json::from_str(file_content)
            .map_err(|_| "Geçersiz yedek dosyası formatı.".to_string())?;

        validate_envelope(&envelope)?;

        let salt =
            hex::decode(&envelope.kdf.salt).map_err(|_| "Geçersiz yedek tuzu.".to_string())?;
        let iv =
            hex::decode(&envelope.cipher.iv).map_err(|_| "Geçersiz yedek IV.".to_string())?;
        let auth_tag = hex::decode(&envelope.cipher.auth_tag)
            .map_err(|_| "Geçersiz yedek doğrulama etiketi.".to_string())?;
        let encrypted_data = hex::decode(&envelope.encrypted_data)
            .map_err(|_| "Geçersiz şifreli yedek verisi.".to_string())?;

        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha256>(
            password.as_bytes(),
            &salt,
            envelope.kdf.iterations,
            &mut key,
        );

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        key.zeroize();

        let mut combined = Vec::with_capacity(encrypted_data.len() + auth_tag.len());
        combined.extend_from_slice(&encrypted_data);
        combined.extend_from_slice(&auth_tag);

        let decrypted = cipher
            .decrypt(Nonce::from_slice(&iv), combined.as_slice())
            .map_err(|_| "Parola hatalı veya yedek dosyası bozuk.".to_string())?;

        let mut decrypted_text = String::from_utf8(decrypted)
            .map_err(|_| "Yedek verisi çözülemedi veya bozulmuş.".to_string())?;

        let payload_res = serde_json::from_str::<BackupPayload>(&decrypted_text)
            .map_err(|_| "Yedek verisi çözülemedi veya bozulmuş.".to_string());
        decrypted_text.zeroize();
        let payload = payload_res?;

        let (imported_count, ungrouped_count) =
            self.account_service.restore_backup(payload, assert_access)?;

        Ok(ImportBackupResult {
            success: true,
            imported_count,
            skipped_count: 0,
            ungrouped_count,
        })
    }
}

fn validate_export_password(password: &str) -> Result<(), String> {
    let len = password.chars().count();
    if len < MIN_BACKUP_PASSWORD_LENGTH {
        return Err(format!(
            "Parola en az {MIN_BACKUP_PASSWORD_LENGTH} karakter olmalıdır."
        ));
    }
    if len > MAX_BACKUP_PASSWORD_LENGTH {
        return Err(format!(
            "Parola en fazla {MAX_BACKUP_PASSWORD_LENGTH} karakter olabilir."
        ));
    }
    Ok(())
}

fn validate_import_password(password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err("Lütfen parolanızı girin.".to_string());
    }
    if password.chars().count() > MAX_BACKUP_PASSWORD_LENGTH {
        return Err("Parola izin verilen uzunluğu aşıyor.".to_string());
    }
    Ok(())
}

fn validate_envelope(envelope: &BackupFileEnvelope) -> Result<(), String> {
    if envelope.format != "authapp-backup-v1"
        || envelope.kdf.algorithm != "pbkdf2"
        || envelope.kdf.hash != "sha256"
        || !(MIN_BACKUP_KDF_ITERATIONS..=MAX_BACKUP_KDF_ITERATIONS)
            .contains(&envelope.kdf.iterations)
        || envelope.kdf.salt.len() != 32
        || envelope.cipher.algorithm != "aes-256-gcm"
        || envelope.cipher.iv.len() != 24
        || envelope.cipher.auth_tag.len() != 32
        || envelope.encrypted_data.len() < 2
        || envelope.encrypted_data.len() > MAX_BACKUP_FILE_BYTES * 2
    {
        return Err("Geçersiz yedek dosyası formatı.".to_string());
    }
    Ok(())
}

fn chrono_like_date_utc(millis: i64) -> String {
    let days_since_epoch = millis.div_euclid(86_400_000);
    // Howard Hinnant's civil_from_days algorithm (UTC)
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    format!("{year:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::database::{AccountRepository, GroupRepository, LocalDatabase};
    use crate::models::{CreateAccountRequest, GroupRecord, TotpAlgorithm};
    use crate::totp::TotpService;
    use crate::vault::{InMemoryAesGcmVault, VaultService};

    fn fixture() -> (LocalDatabase, GroupRepository, AccountService, BackupService) {
        let db = LocalDatabase::open_in_memory().unwrap();
        let groups = GroupRepository::new(db.connection());
        let vault: Arc<dyn VaultService> = Arc::new(InMemoryAesGcmVault::new());
        let accounts = AccountService::new(
            AccountRepository::new(db.connection()),
            vault,
            TotpService,
            Some(groups.clone()),
        );
        let backup = BackupService::new(accounts.clone());
        (db, groups, accounts, backup)
    }

    #[test]
    fn round_trips_encrypted_grouped_and_ungrouped_accounts() {
        let (_db1, src_groups, src_accounts, src_backup) = fixture();
        let (_db2, dst_groups, dst_accounts, dst_backup) = fixture();

        let group = GroupRecord {
            id: "a4ff59c6-2df2-4136-86ba-78bd57a617d8".to_string(),
            name: "Fixture group".to_string(),
            sort_order: 0,
            created_at: 1,
            updated_at: 1,
        };
        src_groups.create(&group).unwrap();
        src_accounts
            .create(
                CreateAccountRequest {
                    issuer: "Fixture".to_string(),
                    account_name: "fixture".to_string(),
                    secret: "JBSWY3DPEHPK3PXP".to_string(),
                    algorithm: TotpAlgorithm::SHA1,
                    digits: 6,
                    period: 30,
                    favorite: false,
                    group_id: Some(group.id.clone()),
                },
                || Ok(()),
            )
            .unwrap();
        src_accounts
            .create(
                CreateAccountRequest {
                    issuer: "Fixture".to_string(),
                    account_name: "ungrouped".to_string(),
                    secret: "JBSWY3DPEHPK3PXP".to_string(),
                    algorithm: TotpAlgorithm::SHA1,
                    digits: 6,
                    period: 30,
                    favorite: false,
                    group_id: None,
                },
                || Ok(()),
            )
            .unwrap();

        let temp_path = std::env::temp_dir().join(format!(
            "authly-backup-test-{}.authapp",
            uuid::Uuid::new_v4()
        ));
        let password = "synthetic-password";
        let export_res = src_backup
            .export_backup_to_path(password, &temp_path, || Ok(()))
            .unwrap();
        assert!(export_res.success);
        assert_eq!(export_res.exported_count, 2);

        let raw_file = fs::read_to_string(&temp_path).unwrap();
        assert!(!raw_file.contains("JBSWY3DPEHPK3PXP"));
        assert!(raw_file.contains("authapp-backup-v1"));

        let import_res = dst_backup
            .import_backup_from_path(password, &temp_path, || Ok(()))
            .unwrap();
        let _ = fs::remove_file(&temp_path);

        assert_eq!(
            import_res,
            ImportBackupResult {
                success: true,
                imported_count: 2,
                skipped_count: 0,
                ungrouped_count: 0,
            }
        );
        let restored_groups = dst_groups.list().unwrap();
        assert_eq!(restored_groups.len(), 1);
        assert_eq!(restored_groups[0].name, "Fixture group");
        let restored_accounts = dst_accounts.list().unwrap();
        assert_eq!(restored_accounts.len(), 2);
        let grouped = restored_accounts
            .iter()
            .find(|a| a.account_name == "fixture")
            .unwrap();
        assert_eq!(grouped.group_id.as_deref(), Some(restored_groups[0].id.as_str()));
    }

    #[test]
    fn fails_import_when_password_is_incorrect_and_handles_v1_missing_groups() {
        let (_db, _groups, dst_accounts, dst_backup) = fixture();
        let payload = BackupPayload {
            version: 1,
            exported_at: 1,
            groups: None,
            accounts: vec![BackupAccountItem {
                issuer: "Fixture".to_string(),
                account_name: "legacy".to_string(),
                secret: "JBSWY3DPEHPK3PXP".to_string(),
                algorithm: TotpAlgorithm::SHA1,
                digits: 6,
                period: 30,
                favorite: false,
                group_id: Some("a4ff59c6-2df2-4136-86ba-78bd57a617d8".to_string()),
            }],
        };
        let password = "synthetic-password";
        let salt = [1u8; 16];
        let iv = [2u8; 12];
        let mut key = [0u8; 32];
        pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, 100_000, &mut key);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let ct = cipher
            .encrypt(
                Nonce::from_slice(&iv),
                serde_json::to_string(&payload).unwrap().as_bytes(),
            )
            .unwrap();
        let split = ct.len() - 16;
        let envelope = BackupFileEnvelope {
            format: "authapp-backup-v1".to_string(),
            kdf: BackupKdf {
                algorithm: "pbkdf2".to_string(),
                iterations: 100_000,
                hash: "sha256".to_string(),
                salt: hex::encode(salt),
            },
            cipher: BackupCipher {
                algorithm: "aes-256-gcm".to_string(),
                iv: hex::encode(iv),
                auth_tag: hex::encode(&ct[split..]),
            },
            encrypted_data: hex::encode(&ct[..split]),
        };
        let json = serde_json::to_string(&envelope).unwrap();

        assert!(dst_backup
            .import_backup_from_str("wrong-password", &json, || Ok(()))
            .is_err());

        let res = dst_backup
            .import_backup_from_str(password, &json, || Ok(()))
            .unwrap();
        assert_eq!(res.imported_count, 1);
        assert_eq!(res.ungrouped_count, 1);
        assert_eq!(dst_accounts.list().unwrap()[0].group_id, None);
    }
}
