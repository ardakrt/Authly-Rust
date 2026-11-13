use pbkdf2::pbkdf2_hmac;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::database::{now_millis, SettingsRepository};
use crate::models::LockStatus;

const CURRENT_PIN_ITERATIONS: u32 = 600_000;
const LEGACY_PIN_ITERATIONS: u32 = 100_000;
const MAX_FAILED_ATTEMPTS_BEFORE_DELAY: i64 = 5;
const BASE_LOCKOUT_MS: i64 = 30_000;
const MAX_LOCKOUT_MS: i64 = 15 * 60_000;

pub type NowFn = Box<dyn Fn() -> i64 + Send + Sync>;
pub type SystemIdleFn = Box<dyn Fn() -> u64 + Send + Sync>;

pub fn default_system_idle_seconds() -> u64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::GetTickCount64;
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
        unsafe {
            let mut lii = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if GetLastInputInfo(&mut lii) != 0 {
                let tick = GetTickCount64() as u32;
                let elapsed_ms = tick.wrapping_sub(lii.dwTime);
                return u64::from(elapsed_ms / 1000);
            }
        }
        0
    }
    #[cfg(not(windows))]
    {
        0
    }
}

pub struct LockService {
    repository: SettingsRepository,
    is_locked: bool,
    last_activity: i64,
    epoch: u64,
    now: NowFn,
    system_idle_seconds: SystemIdleFn,
}

impl LockService {
    pub fn new(repository: SettingsRepository) -> Result<Self, String> {
        Self::with_clocks(
            repository,
            Box::new(now_millis),
            Box::new(default_system_idle_seconds),
        )
    }

    pub fn with_clocks(
        repository: SettingsRepository,
        now: NowFn,
        system_idle_seconds: SystemIdleFn,
    ) -> Result<Self, String> {
        let has_pin = repository.get("pinHash")?.is_some();
        let last_activity = (now)();
        Ok(Self {
            repository,
            is_locked: has_pin,
            last_activity,
            epoch: 0,
            now,
            system_idle_seconds,
        })
    }

    pub fn get_status(&mut self) -> Result<(LockStatus, bool), String> {
        let changed = self.check_auto_lock()?;
        let status = self.snapshot()?;
        Ok((status, changed))
    }

    pub fn snapshot(&self) -> Result<LockStatus, String> {
        let pin_hash = self.repository.get("pinHash")?;
        let is_pin_set = pin_hash.is_some();
        let timeout_raw = self
            .repository
            .get("autoLockTimeout")?
            .unwrap_or_else(|| "5".to_string());
        let auto_lock_timeout = timeout_raw.parse::<u64>().unwrap_or(5);

        Ok(LockStatus {
            is_pin_set,
            is_locked: self.is_locked && is_pin_set,
            auto_lock_timeout,
        })
    }

    pub fn record_activity(&mut self) -> Result<Option<LockStatus>, String> {
        let changed = self.check_auto_lock()?;
        if self.snapshot()?.is_locked {
            return Err("Uygulama kilitli. Lütfen PIN girin.".to_string());
        }
        let _ = changed;
        self.last_activity = (self.now)();
        Ok(None)
    }

    pub fn check_auto_lock(&mut self) -> Result<bool, String> {
        let status = self.snapshot()?;
        if status.is_pin_set && !status.is_locked && status.auto_lock_timeout > 0 {
            let idle_ms = ((self.now)() - self.last_activity)
                .max((self.system_idle_seconds)() as i64 * 1000);
            if idle_ms >= (status.auto_lock_timeout as i64) * 60_000 {
                return self.lock_app();
            }
        }
        Ok(false)
    }

    pub fn capture_session(&mut self) -> Result<u64, String> {
        self.assert_not_locked()?;
        Ok(self.epoch)
    }

    pub fn assert_session(&mut self, expected_epoch: u64) -> Result<(), String> {
        self.assert_not_locked()?;
        if self.epoch != expected_epoch {
            return Err("Kilit oturumu değişti. İşlemi yeniden başlatın.".to_string());
        }
        Ok(())
    }

    pub fn assert_not_locked(&mut self) -> Result<(), String> {
        self.check_auto_lock()?;
        if self.snapshot()?.is_locked {
            return Err("Uygulama kilitli. Lütfen PIN girin.".to_string());
        }
        Ok(())
    }

    pub fn verify_pin(&mut self, pin: &str) -> Result<bool, String> {
        if pin.is_empty() {
            return Err("Lütfen PIN girin.".to_string());
        }
        let pin_hash = self
            .repository
            .get("pinHash")?
            .ok_or_else(|| "PIN oluşturulmamış.".to_string())?;
        let pin_salt = self
            .repository
            .get("pinSalt")?
            .ok_or_else(|| "PIN oluşturulmamış.".to_string())?;

        let now = (self.now)();
        let lockout_until: i64 = self
            .repository
            .get("pinLockoutUntil")?
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);

        if lockout_until > now {
            let remaining_seconds = ((lockout_until - now + 999) / 1000).max(1);
            return Err(format!(
                "Çok fazla hatalı deneme. {remaining_seconds} saniye sonra tekrar deneyin."
            ));
        }

        if lockout_until > 0 {
            self.repository.delete("pinLockoutUntil")?;
        }

        let iterations: u32 = self
            .repository
            .get("pinIterations")?
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(LEGACY_PIN_ITERATIONS);

        let salt = hex::decode(&pin_salt).map_err(|_| "PIN tuzu bozuk.".to_string())?;
        let expected_hash =
            hex::decode(&pin_hash).map_err(|_| "PIN özeti bozuk.".to_string())?;

        let mut derived_hash = [0u8; 32];
        pbkdf2_hmac::<Sha256>(pin.as_bytes(), &salt, iterations, &mut derived_hash);

        let matches = derived_hash.len() == expected_hash.len()
            && bool::from(derived_hash.ct_eq(&expected_hash));

        if !matches {
            let failed_attempts = self
                .repository
                .get("pinFailedAttempts")?
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0)
                + 1;
            self.repository
                .set("pinFailedAttempts", &failed_attempts.to_string())?;

            if failed_attempts >= MAX_FAILED_ATTEMPTS_BEFORE_DELAY {
                let delay_level = ((failed_attempts - MAX_FAILED_ATTEMPTS_BEFORE_DELAY)
                    / MAX_FAILED_ATTEMPTS_BEFORE_DELAY) as u32;
                let multiplier = 1i64.checked_shl(delay_level.min(20)).unwrap_or(i64::MAX);
                let lockout_ms = BASE_LOCKOUT_MS.saturating_mul(multiplier).min(MAX_LOCKOUT_MS);
                self.repository
                    .set("pinLockoutUntil", &(now + lockout_ms).to_string())?;
                let wait_secs = (lockout_ms + 999) / 1000;
                return Err(format!(
                    "Çok fazla hatalı deneme. {wait_secs} saniye bekleyin."
                ));
            }

            return Err("PIN hatalı.".to_string());
        }

        self.clear_failed_attempts()?;
        if iterations < CURRENT_PIN_ITERATIONS {
            self.store_pin(pin)?;
        }
        self.is_locked = false;
        self.last_activity = (self.now)();
        Ok(true)
    }

    pub fn set_pin(&mut self, pin: &str, current_pin: Option<&str>) -> Result<LockStatus, String> {
        let len = pin.chars().count();
        if len < 4 {
            return Err("PIN en az 4 haneli olmalıdır.".to_string());
        }
        if len > 12 {
            return Err("PIN en fazla 12 haneli olmalıdır.".to_string());
        }

        if self.repository.get("pinHash")?.is_some() {
            let existing = current_pin.ok_or_else(|| "Mevcut PIN zorunludur.".to_string())?;
            self.verify_pin(existing)?;
        }

        self.store_pin(pin)?;
        self.clear_failed_attempts()?;
        self.is_locked = false;
        self.last_activity = (self.now)();
        self.epoch += 1;
        self.snapshot()
    }

    pub fn remove_pin(&mut self, current_pin: &str) -> Result<LockStatus, String> {
        self.verify_pin(current_pin)?;
        self.repository.delete("pinHash")?;
        self.repository.delete("pinSalt")?;
        self.repository.delete("pinIterations")?;
        self.clear_failed_attempts()?;
        self.is_locked = false;
        self.epoch += 1;
        self.snapshot()
    }

    pub fn lock_app(&mut self) -> Result<bool, String> {
        if self.repository.get("pinHash")?.is_some() && !self.is_locked {
            self.is_locked = true;
            self.epoch += 1;
            return Ok(true);
        }
        Ok(false)
    }

    fn store_pin(&self, pin: &str) -> Result<(), String> {
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        let mut hash = [0u8; 32];
        pbkdf2_hmac::<Sha256>(pin.as_bytes(), &salt, CURRENT_PIN_ITERATIONS, &mut hash);
        self.repository.set("pinSalt", &hex::encode(salt))?;
        self.repository.set("pinHash", &hex::encode(hash))?;
        self.repository
            .set("pinIterations", &CURRENT_PIN_ITERATIONS.to_string())?;
        Ok(())
    }

    fn clear_failed_attempts(&self) -> Result<(), String> {
        self.repository.delete("pinFailedAttempts")?;
        self.repository.delete("pinLockoutUntil")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::database::LocalDatabase;

    fn fixture() -> (LocalDatabase, Arc<Mutex<i64>>, Arc<Mutex<u64>>, LockService) {
        let db = LocalDatabase::open_in_memory().unwrap();
        let repo = SettingsRepository::new(db.connection());
        let now = Arc::new(Mutex::new(1_000i64));
        let idle = Arc::new(Mutex::new(0u64));
        let now_clone = Arc::clone(&now);
        let idle_clone = Arc::clone(&idle);
        let service = LockService::with_clocks(
            repo,
            Box::new(move || *now_clone.lock().unwrap()),
            Box::new(move || *idle_clone.lock().unwrap()),
        )
        .unwrap();
        (db, now, idle, service)
    }

    #[test]
    fn enforces_inactivity_and_os_idle_time() {
        let (_db, now, idle, mut service) = fixture();
        service.set_pin("1234", None).unwrap();

        *now.lock().unwrap() += 299_999;
        let (status, _) = service.get_status().unwrap();
        assert!(!status.is_locked);

        *now.lock().unwrap() += 1;
        assert!(service.check_auto_lock().unwrap());
        assert!(!service.check_auto_lock().unwrap());
        assert!(service.assert_not_locked().is_err());

        service.verify_pin("1234").unwrap();
        *idle.lock().unwrap() = 300;
        assert!(service.record_activity().is_err());
        assert!(service.snapshot().unwrap().is_locked);
    }

    #[test]
    fn revokes_captured_session_when_locked_and_unlocked() {
        let (_db, _now, _idle, mut service) = fixture();
        service.set_pin("1234", None).unwrap();
        let epoch = service.capture_session().unwrap();
        service.lock_app().unwrap();
        service.verify_pin("1234").unwrap();
        assert!(service.assert_session(epoch).is_err());
        let next_epoch = service.capture_session().unwrap();
        assert!(service.assert_session(next_epoch).is_ok());
    }

    #[test]
    fn rate_limits_failed_pin_attempts() {
        let (_db, now, _idle, mut service) = fixture();
        service.set_pin("1234", None).unwrap();
        service.lock_app().unwrap();

        for _ in 0..4 {
            assert!(service.verify_pin("9999").is_err());
        }
        let err = service.verify_pin("9999").unwrap_err();
        assert!(err.contains("Çok fazla hatalı deneme"));
        assert!(service.verify_pin("1234").is_err());

        *now.lock().unwrap() += 30_001;
        assert!(service.verify_pin("1234").unwrap());
    }
}
