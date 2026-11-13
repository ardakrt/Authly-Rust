use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TotpAlgorithm {
    SHA1,
    SHA256,
    SHA512,
}

impl Default for TotpAlgorithm {
    fn default() -> Self {
        Self::SHA1
    }
}

impl TotpAlgorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SHA1 => "SHA1",
            Self::SHA256 => "SHA256",
            Self::SHA512 => "SHA512",
        }
    }

    pub fn from_str_strict(value: &str) -> Result<Self, String> {
        match value.to_ascii_uppercase().as_str() {
            "SHA1" => Ok(Self::SHA1),
            "SHA256" => Ok(Self::SHA256),
            "SHA512" => Ok(Self::SHA512),
            _ => Err("Desteklenmeyen TOTP algoritması.".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmptyRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub app_name: String,
    pub app_version: String,
    pub platform: String,
    pub packaged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub id: String,
    pub issuer: String,
    pub account_name: String,
    pub algorithm: TotpAlgorithm,
    pub digits: u32,
    pub period: u32,
    pub favorite: bool,
    pub group_id: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRecord {
    pub id: String,
    pub issuer: String,
    pub account_name: String,
    pub encrypted_secret: Vec<u8>,
    pub algorithm: TotpAlgorithm,
    pub digits: u32,
    pub period: u32,
    pub favorite: bool,
    pub group_id: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl AccountRecord {
    pub fn to_dto(&self) -> AccountDto {
        AccountDto {
            id: self.id.clone(),
            issuer: self.issuer.clone(),
            account_name: self.account_name.clone(),
            algorithm: self.algorithm,
            digits: self.digits,
            period: self.period,
            favorite: self.favorite,
            group_id: self.group_id.clone(),
            sort_order: self.sort_order,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

fn default_digits() -> u32 {
    6
}

fn default_period() -> u32 {
    30
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAccountRequest {
    pub issuer: String,
    pub account_name: String,
    pub secret: String,
    #[serde(default)]
    pub algorithm: TotpAlgorithm,
    #[serde(default = "default_digits")]
    pub digits: u32,
    #[serde(default = "default_period")]
    pub period: u32,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportAccountsRequest {
    pub accounts: Vec<CreateAccountRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAccountRequest {
    pub id: String,
    pub issuer: String,
    pub account_name: String,
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub algorithm: TotpAlgorithm,
    #[serde(default = "default_digits")]
    pub digits: u32,
    #[serde(default = "default_period")]
    pub period: u32,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteAccountRequest {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpCode {
    pub account_id: String,
    pub code: String,
    pub remaining: u32,
    pub period: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParseOtpAuthUriRequest {
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CopyTotpRequest {
    pub account_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupRecord {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportBackupRequest {
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBackupResult {
    pub success: bool,
    pub file_path: Option<String>,
    pub exported_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportBackupRequest {
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBackupResult {
    pub success: bool,
    pub imported_count: usize,
    pub skipped_count: usize,
    pub ungrouped_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupAccountItem {
    pub issuer: String,
    pub account_name: String,
    pub secret: String,
    #[serde(default)]
    pub algorithm: TotpAlgorithm,
    #[serde(default = "default_digits")]
    pub digits: u32,
    #[serde(default = "default_period")]
    pub period: u32,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupPayload {
    pub version: u8,
    pub exported_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<GroupRecord>>,
    pub accounts: Vec<BackupAccountItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupKdf {
    pub algorithm: String,
    pub iterations: u32,
    pub hash: String,
    pub salt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupCipher {
    pub algorithm: String,
    pub iv: String,
    pub auth_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupFileEnvelope {
    pub format: String,
    pub kdf: BackupKdf,
    pub cipher: BackupCipher,
    pub encrypted_data: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguagePreference {
    Tr,
    En,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: ThemePreference,
    pub language: LanguagePreference,
    pub close_to_tray: bool,
    pub start_minimized: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateSettingsRequest {
    pub theme: Option<ThemePreference>,
    pub language: Option<LanguagePreference>,
    pub close_to_tray: Option<bool>,
    pub start_minimized: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockStatus {
    pub is_pin_set: bool,
    pub is_locked: bool,
    pub auto_lock_timeout: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerifyPinRequest {
    pub pin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetPinRequest {
    pub pin: String,
    pub current_pin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemovePinRequest {
    pub current_pin: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdatePhase {
    Idle,
    Checking,
    Available,
    Downloading,
    Downloaded,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateState {
    pub phase: UpdatePhase,
    pub current_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_per_second: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
