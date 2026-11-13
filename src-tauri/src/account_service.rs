use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use uuid::Uuid;

use crate::database::{now_millis, AccountRepository, GroupRepository};
use crate::models::{
    AccountDto, AccountRecord, BackupPayload, CreateAccountRequest, GroupRecord,
    ImportAccountsRequest, TotpCode, UpdateAccountRequest,
};
use crate::totp::{normalize_base32_secret, TotpService};
use crate::vault::VaultService;

pub const MAX_ACCOUNT_IMPORT_SIZE: usize = 100;
pub const MAX_BACKUP_ACCOUNTS: usize = 10_000;

fn validate_uuid_str(value: &str) -> Result<String, String> {
    let parsed = Uuid::parse_str(value).map_err(|_| "Geçersiz UUID değeri.".to_string())?;
    Ok(parsed.to_string())
}

fn validate_account_params(
    issuer: &str,
    account_name: &str,
    digits: u32,
    period: u32,
    group_id: Option<&str>,
) -> Result<(String, String, Option<String>), String> {
    let issuer = issuer.trim().to_string();
    let account_name = account_name.trim().to_string();
    if issuer.is_empty() || issuer.chars().count() > 120 {
        return Err("Servis adı 1 ile 120 karakter arasında olmalıdır.".to_string());
    }
    if account_name.is_empty() || account_name.chars().count() > 240 {
        return Err("Hesap adı 1 ile 240 karakter arasında olmalıdır.".to_string());
    }
    if !(6..=10).contains(&digits) {
        return Err("Basamak sayısı 6 ile 10 arasında olmalıdır.".to_string());
    }
    if !(5..=300).contains(&period) {
        return Err("Süre 5 ile 300 saniye arasında olmalıdır.".to_string());
    }
    let normalized_group = match group_id {
        Some(gid) => Some(validate_uuid_str(gid)?),
        None => None,
    };
    Ok((issuer, account_name, normalized_group))
}

#[derive(Clone)]
pub struct AccountService {
    repository: AccountRepository,
    vault: Arc<dyn VaultService>,
    totp_service: TotpService,
    group_repository: Option<GroupRepository>,
}

impl AccountService {
    pub fn new(
        repository: AccountRepository,
        vault: Arc<dyn VaultService>,
        totp_service: TotpService,
        group_repository: Option<GroupRepository>,
    ) -> Self {
        Self {
            repository,
            vault,
            totp_service,
            group_repository,
        }
    }

    pub fn list(&self) -> Result<Vec<AccountDto>, String> {
        Ok(self
            .repository
            .list()?
            .into_iter()
            .map(|r| r.to_dto())
            .collect())
    }

    pub fn get_totp_codes(&self) -> Result<Vec<TotpCode>, String> {
        let accounts = self.repository.list()?;
        let mut codes = Vec::new();
        for account in accounts {
            if let Ok(code) = self.get_totp_code(&account.id) {
                codes.push(code);
            }
        }
        Ok(codes)
    }

    pub fn get_totp_code(&self, id: &str) -> Result<TotpCode, String> {
        let account_id = validate_uuid_str(id)?;
        let (secret, account) = self.get_totp_secret(&account_id)?;
        let timestamp_ms = now_millis().max(0) as u64;
        let value = self.totp_service.generate(
            &secret,
            account.algorithm,
            account.digits,
            account.period,
            timestamp_ms,
        )?;
        Ok(TotpCode {
            account_id,
            code: value.code,
            remaining: value.remaining,
            period: value.period,
        })
    }

    pub fn get_totp_secret(&self, id: &str) -> Result<(String, AccountRecord), String> {
        let account = self
            .repository
            .get_by_id(id)?
            .ok_or_else(|| "Account not found.".to_string())?;
        let decrypted = self.vault.decrypt_secret(&account.encrypted_secret)?;
        if let Some(ref re_encrypted) = decrypted.re_encrypted {
            self.repository.update_encrypted_secret(id, re_encrypted)?;
        }
        Ok((decrypted.secret, account))
    }

    pub fn create<F>(
        &self,
        input: CreateAccountRequest,
        assert_can_write: F,
    ) -> Result<AccountDto, String>
    where
        F: Fn() -> Result<(), String>,
    {
        let (issuer, account_name, group_id) = validate_account_params(
            &input.issuer,
            &input.account_name,
            input.digits,
            input.period,
            input.group_id.as_deref(),
        )?;
        let normalized_secret = normalize_base32_secret(&input.secret)?;
        let encrypted_secret = self.vault.encrypt_secret(&normalized_secret)?;
        let now = now_millis();
        let sort_order = self.repository.list()?.len() as i64;
        let record = AccountRecord {
            id: Uuid::new_v4().to_string(),
            issuer,
            account_name,
            encrypted_secret,
            algorithm: input.algorithm,
            digits: input.digits,
            period: input.period,
            favorite: input.favorite,
            group_id,
            sort_order,
            created_at: now,
            updated_at: now,
        };
        let dto = record.to_dto();
        assert_can_write()?;
        self.repository.create(&record)?;
        Ok(dto)
    }

    pub fn import_accounts<F>(
        &self,
        input: ImportAccountsRequest,
        assert_can_write: F,
    ) -> Result<Vec<AccountDto>, String>
    where
        F: Fn() -> Result<(), String>,
    {
        if input.accounts.is_empty() || input.accounts.len() > MAX_ACCOUNT_IMPORT_SIZE {
            return Err("İçe aktarılacak hesap sayısı geçersiz.".to_string());
        }

        let now = now_millis();
        let base_sort_order = self.repository.list()?.len() as i64;
        let mut records = Vec::with_capacity(input.accounts.len());

        for (index, item) in input.accounts.into_iter().enumerate() {
            let (issuer, account_name, group_id) = validate_account_params(
                &item.issuer,
                &item.account_name,
                item.digits,
                item.period,
                item.group_id.as_deref(),
            )?;
            let normalized_secret = normalize_base32_secret(&item.secret)?;
            let encrypted_secret = self.vault.encrypt_secret(&normalized_secret)?;
            records.push(AccountRecord {
                id: Uuid::new_v4().to_string(),
                issuer,
                account_name,
                encrypted_secret,
                algorithm: item.algorithm,
                digits: item.digits,
                period: item.period,
                favorite: item.favorite,
                group_id,
                sort_order: base_sort_order + index as i64,
                created_at: now,
                updated_at: now,
            });
        }

        let dtos: Vec<AccountDto> = records.iter().map(AccountRecord::to_dto).collect();
        assert_can_write()?;
        self.repository.create_many(&records)?;
        Ok(dtos)
    }

    pub fn list_groups(&self) -> Result<Vec<GroupRecord>, String> {
        match &self.group_repository {
            Some(repo) => repo.list(),
            None => Ok(Vec::new()),
        }
    }

    pub fn restore_backup<F>(
        &self,
        payload: BackupPayload,
        assert_can_write: F,
    ) -> Result<(usize, usize), String>
    where
        F: Fn() -> Result<(), String>,
    {
        if payload.version != 1 && payload.version != 2 {
            return Err("Desteklenmeyen yedek sürümü.".to_string());
        }
        if payload.exported_at <= 0 || payload.accounts.len() > MAX_BACKUP_ACCOUNTS {
            return Err("Geçersiz yedek içeriği.".to_string());
        }
        if payload.version == 1 && payload.groups.is_some() {
            return Err("Geçersiz v1 yedek içeriği.".to_string());
        }

        let existing_groups = self.list_groups()?;
        let mut groups_to_create: Vec<GroupRecord> = Vec::new();
        let mut group_map: HashMap<String, String> = HashMap::new();

        if payload.version == 2 {
            let groups = payload
                .groups
                .ok_or_else(|| "v2 yedeğinde grup listesi eksik.".to_string())?;
            if groups.len() > MAX_BACKUP_ACCOUNTS {
                return Err("Yedekteki grup sayısı sınırı aşıyor.".to_string());
            }
            let mut seen_ids = HashSet::new();
            for group in groups {
                let group_id = validate_uuid_str(&group.id)?;
                let name = group.name.trim().to_string();
                if name.is_empty() || name.chars().count() > 120 || group.sort_order < 0 {
                    return Err("Yedekte geçersiz grup bilgisi var.".to_string());
                }
                if !seen_ids.insert(group_id.clone()) {
                    return Err("Yedekte yinelenen grup bilgisi var.".to_string());
                }
                let existing = existing_groups
                    .iter()
                    .chain(groups_to_create.iter())
                    .find(|item| item.name == name)
                    .cloned();
                let restored = match existing {
                    Some(found) => found,
                    None => {
                        let created = GroupRecord {
                            id: Uuid::new_v4().to_string(),
                            name,
                            sort_order: (existing_groups.len() + groups_to_create.len()) as i64,
                            created_at: group.created_at.max(1),
                            updated_at: group.updated_at.max(1),
                        };
                        groups_to_create.push(created.clone());
                        created
                    }
                };
                group_map.insert(group_id, restored.id);
            }
        }

        if !groups_to_create.is_empty() && self.group_repository.is_none() {
            return Err("Grup deposu kullanılamıyor.".to_string());
        }

        let mut ungrouped_count = 0usize;
        let base_sort_order = self.repository.list()?.len() as i64;
        let now = now_millis();
        let mut records = Vec::with_capacity(payload.accounts.len());

        for (index, account) in payload.accounts.into_iter().enumerate() {
            let (issuer, account_name, mut group_id) = validate_account_params(
                &account.issuer,
                &account.account_name,
                account.digits,
                account.period,
                account.group_id.as_deref(),
            )?;
            let normalized_secret = normalize_base32_secret(&account.secret)?;

            if let Some(ref gid) = group_id {
                let mapped = group_map.get(gid).cloned();
                if payload.version == 2 && mapped.is_none() {
                    return Err("Yedekte eksik grup bilgisi var.".to_string());
                }
                if payload.version == 1
                    && !existing_groups.iter().any(|item| &item.id == gid)
                {
                    group_id = None;
                    ungrouped_count += 1;
                } else if let Some(mapped_id) = mapped {
                    group_id = Some(mapped_id);
                }
            }

            let encrypted_secret = self.vault.encrypt_secret(&normalized_secret)?;
            assert_can_write()?;

            records.push(AccountRecord {
                id: Uuid::new_v4().to_string(),
                issuer,
                account_name,
                encrypted_secret,
                algorithm: account.algorithm,
                digits: account.digits,
                period: account.period,
                favorite: account.favorite,
                group_id,
                sort_order: base_sort_order + index as i64,
                created_at: now,
                updated_at: now,
            });
        }

        assert_can_write()?;
        self.repository
            .restore_groups_and_accounts(&groups_to_create, &records)?;
        Ok((records.len(), ungrouped_count))
    }

    pub fn update<F>(
        &self,
        input: UpdateAccountRequest,
        assert_can_write: F,
    ) -> Result<AccountDto, String>
    where
        F: Fn() -> Result<(), String>,
    {
        let id = validate_uuid_str(&input.id)?;
        let (issuer, account_name, group_id) = validate_account_params(
            &input.issuer,
            &input.account_name,
            input.digits,
            input.period,
            input.group_id.as_deref(),
        )?;
        let current = self
            .repository
            .get_by_id(&id)?
            .ok_or_else(|| "Account not found.".to_string())?;

        let encrypted_secret = match input.secret {
            Some(ref raw_secret) => {
                let normalized = normalize_base32_secret(raw_secret)?;
                self.vault.encrypt_secret(&normalized)?
            }
            None => current.encrypted_secret,
        };

        let updated = AccountRecord {
            id: current.id,
            issuer,
            account_name,
            encrypted_secret,
            algorithm: input.algorithm,
            digits: input.digits,
            period: input.period,
            favorite: input.favorite,
            group_id,
            sort_order: current.sort_order,
            created_at: current.created_at,
            updated_at: now_millis(),
        };

        let dto = updated.to_dto();
        assert_can_write()?;
        self.repository.update(&updated)?;
        Ok(dto)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let id = validate_uuid_str(id)?;
        if !self.repository.delete(&id)? {
            return Err("Account not found.".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::LocalDatabase;
    use crate::models::TotpAlgorithm;
    use crate::vault::InMemoryAesGcmVault;

    fn sample_input(account_name: &str) -> CreateAccountRequest {
        CreateAccountRequest {
            issuer: "Test Service".to_string(),
            account_name: account_name.to_string(),
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            algorithm: TotpAlgorithm::SHA1,
            digits: 6,
            period: 30,
            favorite: false,
            group_id: None,
        }
    }

    fn setup() -> (LocalDatabase, AccountRepository, GroupRepository, AccountService) {
        let db = LocalDatabase::open_in_memory().unwrap();
        let repo = AccountRepository::new(db.connection());
        let groups = GroupRepository::new(db.connection());
        let vault: Arc<dyn VaultService> = Arc::new(InMemoryAesGcmVault::new());
        let service = AccountService::new(repo.clone(), vault, TotpService, Some(groups.clone()));
        (db, repo, groups, service)
    }

    #[test]
    fn rejects_malformed_secrets_on_create_and_update() {
        let (_db, _repo, _groups, service) = setup();
        let mut bad = sample_input("local@example.test");
        bad.secret = "INVALID!".to_string();
        assert!(service.create(bad, || Ok(())).is_err());
        assert!(service.list().unwrap().is_empty());

        let created = service.create(sample_input("local@example.test"), || Ok(())).unwrap();
        assert!(service
            .update(
                UpdateAccountRequest {
                    id: created.id.clone(),
                    issuer: created.issuer.clone(),
                    account_name: created.account_name.clone(),
                    secret: Some("INVALID!".to_string()),
                    algorithm: created.algorithm,
                    digits: created.digits,
                    period: created.period,
                    favorite: created.favorite,
                    group_id: created.group_id.clone(),
                },
                || Ok(())
            )
            .is_err());
        assert_eq!(service.get_totp_code(&created.id).unwrap().account_id, created.id);
    }

    #[test]
    fn isolates_broken_accounts_when_generating_codes() {
        let (_db, repo, _groups, service) = setup();
        let healthy = service.create(sample_input("healthy"), || Ok(())).unwrap();
        let broken = service.create(sample_input("broken"), || Ok(())).unwrap();
        repo.update_encrypted_secret(&broken.id, b"corrupted").unwrap();

        let codes = service.get_totp_codes().unwrap();
        assert_eq!(codes.len(), 1);
        assert_eq!(codes[0].account_id, healthy.id);
        assert!(service.get_totp_code(&broken.id).is_err());
        assert_eq!(service.list().unwrap().len(), 2);
    }

    #[test]
    fn imports_atomically_and_rolls_back_on_error() {
        let (_db, _repo, _groups, service) = setup();
        let existing = service.create(sample_input("existing"), || Ok(())).unwrap();

        let mut bad_group = sample_input("second");
        bad_group.group_id = Some("a4ff59c6-2df2-4136-86ba-78bd57a617d8".to_string());
        assert!(service
            .import_accounts(
                ImportAccountsRequest {
                    accounts: vec![sample_input("first"), bad_group],
                },
                || Ok(()),
            )
            .is_err());
        assert_eq!(service.list().unwrap().len(), 1);
        assert_eq!(service.list().unwrap()[0].id, existing.id);

        let imported = service
            .import_accounts(
                ImportAccountsRequest {
                    accounts: vec![sample_input("a"), sample_input("b")],
                },
                || Ok(()),
            )
            .unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].sort_order, 1);
        assert_eq!(imported[1].sort_order, 2);
    }

    #[test]
    fn writes_nothing_when_lock_guard_fails() {
        let (_db, _repo, _groups, service) = setup();
        let res = service.create(sample_input("locked"), || Err("Locked".to_string()));
        assert!(res.is_err());
        assert!(service.list().unwrap().is_empty());
    }
}
