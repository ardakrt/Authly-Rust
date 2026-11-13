pub mod account_service;
pub mod backup_service;
pub mod database;
pub mod lock_service;
pub mod models;
pub mod settings_service;
pub mod totp;
pub mod update_service;
pub mod vault;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

use crate::account_service::AccountService;
use crate::backup_service::BackupService;
use crate::database::{
    AccountRepository, GroupRepository, LocalDatabase, SettingsRepository,
};
use crate::lock_service::LockService;
use crate::models::{
    AccountDto, AppSettings, CopyTotpRequest, CreateAccountRequest, DeleteAccountRequest,
    EmptyRequest, ExportBackupRequest, ExportBackupResult, ImportAccountsRequest,
    ImportBackupRequest, ImportBackupResult, LockStatus, ParseOtpAuthUriRequest, RemovePinRequest,
    RuntimeInfo, SetPinRequest, TotpCode, UpdateAccountRequest, UpdateInfo, UpdatePhase,
    UpdateSettingsRequest, UpdateState, VerifyPinRequest,
};
use crate::settings_service::SettingsService;
use crate::totp::TotpService;
use crate::update_service::{fetch_latest_github_release, is_trusted_release_url, UpdateService};
use crate::vault::{
    migrate_legacy_electron_db, InMemoryAesGcmVault, KeyringAesGcmVault, VaultService,
};

const LOCK_STATUS_EVENT: &str = "lock:status-changed";
const UPDATE_STATE_EVENT: &str = "app:update-state";

pub struct AppState {
    pub account_service: AccountService,
    pub totp_service: TotpService,
    pub backup_service: BackupService,
    pub settings_service: SettingsService,
    pub lock_service: Arc<Mutex<LockService>>,
    pub update_service: Arc<Mutex<UpdateService>>,
    pub is_quitting: Arc<AtomicBool>,
    pub window_ready: Arc<AtomicBool>,
    pub suppress_initial_show: bool,
}

fn emit_lock_status(app: &AppHandle, status: &LockStatus) {
    let _ = app.emit(LOCK_STATUS_EVENT, status);
}

fn emit_update_state(app: &AppHandle, state: &UpdateState) {
    let _ = app.emit(UPDATE_STATE_EVENT, state);
}

fn lock_guard_check(lock_service: &Arc<Mutex<LockService>>, expected_epoch: Option<u64>) -> Result<(), String> {
    let mut guard = lock_service
        .lock()
        .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
    match expected_epoch {
        Some(epoch) => guard.assert_session(epoch),
        None => guard.assert_not_locked(),
    }
}

#[tauri::command]
fn get_runtime_info(app: AppHandle, request: EmptyRequest) -> Result<RuntimeInfo, String> {
    let _ = request;
    let info = app.package_info();
    let platform = if cfg!(target_os = "windows") {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    Ok(RuntimeInfo {
        app_name: info.name.clone(),
        app_version: info.version.to_string(),
        platform: platform.to_string(),
        packaged: !cfg!(debug_assertions),
    })
}

#[tauri::command]
fn list_accounts(state: State<'_, AppState>, request: EmptyRequest) -> Result<Vec<AccountDto>, String> {
    let _ = request;
    lock_guard_check(&state.lock_service, None)?;
    state.account_service.list()
}

#[tauri::command]
fn create_account(
    state: State<'_, AppState>,
    request: CreateAccountRequest,
) -> Result<AccountDto, String> {
    let epoch = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.capture_session()?
    };
    let lock_ref = Arc::clone(&state.lock_service);
    state
        .account_service
        .create(request, move || lock_guard_check(&lock_ref, Some(epoch)))
}

#[tauri::command]
fn import_accounts(
    state: State<'_, AppState>,
    request: ImportAccountsRequest,
) -> Result<Vec<AccountDto>, String> {
    let epoch = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.capture_session()?
    };
    let lock_ref = Arc::clone(&state.lock_service);
    state
        .account_service
        .import_accounts(request, move || lock_guard_check(&lock_ref, Some(epoch)))
}

#[tauri::command]
fn update_account(
    state: State<'_, AppState>,
    request: UpdateAccountRequest,
) -> Result<AccountDto, String> {
    let epoch = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.capture_session()?
    };
    let lock_ref = Arc::clone(&state.lock_service);
    state
        .account_service
        .update(request, move || lock_guard_check(&lock_ref, Some(epoch)))
}

#[tauri::command]
fn delete_account(state: State<'_, AppState>, request: DeleteAccountRequest) -> Result<(), String> {
    lock_guard_check(&state.lock_service, None)?;
    state.account_service.delete(&request.id)
}

#[tauri::command]
fn get_totp_codes(
    state: State<'_, AppState>,
    request: EmptyRequest,
) -> Result<Vec<TotpCode>, String> {
    let _ = request;
    let epoch = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.capture_session()?
    };
    let codes = state.account_service.get_totp_codes()?;
    lock_guard_check(&state.lock_service, Some(epoch))?;
    Ok(codes)
}

#[tauri::command]
fn parse_otp_auth_uri(
    state: State<'_, AppState>,
    request: ParseOtpAuthUriRequest,
) -> Result<CreateAccountRequest, String> {
    state.totp_service.parse_uri(request.uri.trim())
}

#[tauri::command]
fn copy_totp(state: State<'_, AppState>, request: CopyTotpRequest) -> Result<(), String> {
    let epoch = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.capture_session()?
    };
    let value = state.account_service.get_totp_code(&request.account_id)?;
    lock_guard_check(&state.lock_service, Some(epoch))?;

    let mut clipboard =
        arboard::Clipboard::new().map_err(|_| "Panoya erişilemedi.".to_string())?;
    clipboard
        .set_text(value.code.clone())
        .map_err(|_| "Kod panoya kopyalanamadı.".to_string())?;

    let expected_code = value.code;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(15));
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if cb.get_text().ok().as_deref() == Some(expected_code.as_str()) {
                let _ = cb.clear();
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn export_backup(
    state: State<'_, AppState>,
    request: ExportBackupRequest,
) -> Result<ExportBackupResult, String> {
    let epoch = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.capture_session()?
    };
    let lock_ref = Arc::clone(&state.lock_service);
    state
        .backup_service
        .export_backup(&request.password, move || lock_guard_check(&lock_ref, Some(epoch)))
}

#[tauri::command]
fn import_backup(
    state: State<'_, AppState>,
    request: ImportBackupRequest,
) -> Result<ImportBackupResult, String> {
    let epoch = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.capture_session()?
    };
    let lock_ref = Arc::clone(&state.lock_service);
    state
        .backup_service
        .import_backup(&request.password, move || lock_guard_check(&lock_ref, Some(epoch)))
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>, request: EmptyRequest) -> Result<AppSettings, String> {
    let _ = request;
    state.settings_service.get_settings()
}

#[tauri::command]
fn update_settings(
    state: State<'_, AppState>,
    request: UpdateSettingsRequest,
) -> Result<AppSettings, String> {
    lock_guard_check(&state.lock_service, None)?;
    state.settings_service.update_settings(request)
}

#[tauri::command]
fn get_lock_status(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EmptyRequest,
) -> Result<LockStatus, String> {
    let _ = request;
    let (status, changed) = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.get_status()?
    };
    if changed {
        state.account_service.clear_vault_cache();
        emit_lock_status(&app, &status);
    }
    Ok(status)
}

#[tauri::command]
fn verify_pin(
    app: AppHandle,
    state: State<'_, AppState>,
    request: VerifyPinRequest,
) -> Result<bool, String> {
    let (verified, status) = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        let verified = guard.verify_pin(&request.pin)?;
        let status = guard.snapshot()?;
        (verified, status)
    };
    emit_lock_status(&app, &status);
    Ok(verified)
}

#[tauri::command]
fn set_pin(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SetPinRequest,
) -> Result<(), String> {
    let status = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.set_pin(&request.pin, request.current_pin.as_deref())?
    };
    emit_lock_status(&app, &status);
    Ok(())
}

#[tauri::command]
fn remove_pin(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RemovePinRequest,
) -> Result<(), String> {
    let status = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.remove_pin(&request.current_pin)?
    };
    emit_lock_status(&app, &status);
    Ok(())
}

#[tauri::command]
fn lock_app(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EmptyRequest,
) -> Result<(), String> {
    let _ = request;
    let maybe_status = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        if guard.lock_app()? {
            Some(guard.snapshot()?)
        } else {
            None
        }
    };
    if let Some(status) = maybe_status {
        state.account_service.clear_vault_cache();
        emit_lock_status(&app, &status);
    }
    Ok(())
}

#[tauri::command]
fn record_activity(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EmptyRequest,
) -> Result<(), String> {
    let _ = request;
    let res = {
        let mut guard = state
            .lock_service
            .lock()
            .map_err(|_| "Kilit servisi kullanılamıyor.".to_string())?;
        guard.record_activity()
    };
    if res.is_err() {
        if let Ok(guard) = state.lock_service.lock() {
            if let Ok(status) = guard.snapshot() {
                if status.is_locked {
                    state.account_service.clear_vault_cache();
                    emit_lock_status(&app, &status);
                }
            }
        }
    }
    res.map(|_| ())
}

#[tauri::command]
async fn check_update(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EmptyRequest,
) -> Result<UpdateInfo, String> {
    let _ = request;
    let (current_version, is_packaged) = {
        let guard = state
            .update_service
            .lock()
            .map_err(|_| "Güncelleme servisi kullanılamıyor.".to_string())?;
        (guard.current_version().to_string(), guard.is_packaged())
    };

    if !is_packaged {
        let error = "Güncelleme denetimi yalnızca kurulu uygulamada kullanılabilir.".to_string();
        let next_state = {
            let mut guard = state
                .update_service
                .lock()
                .map_err(|_| "Güncelleme servisi kullanılamıyor.".to_string())?;
            guard.set_state(UpdateState {
                phase: UpdatePhase::Error,
                current_version: current_version.clone(),
                latest_version: None,
                progress: None,
                bytes_per_second: None,
                error: Some(error.clone()),
            })
        };
        emit_update_state(&app, &next_state);
        return Ok(UpdateInfo {
            has_update: false,
            current_version: current_version.clone(),
            latest_version: current_version,
            release_url: None,
            release_notes: None,
            published_at: None,
            error: Some(error),
        });
    }

    {
        let mut guard = state
            .update_service
            .lock()
            .map_err(|_| "Güncelleme servisi kullanılamıyor.".to_string())?;
        let checking = guard.set_state(UpdateState {
            phase: UpdatePhase::Checking,
            current_version: current_version.clone(),
            latest_version: None,
            progress: None,
            bytes_per_second: None,
            error: None,
        });
        emit_update_state(&app, &checking);
    }

    match fetch_latest_github_release(&current_version).await {
        Ok(info) => {
            let next_state = {
                let mut guard = state
                    .update_service
                    .lock()
                    .map_err(|_| "Güncelleme servisi kullanılamıyor.".to_string())?;
                guard.set_state(UpdateState {
                    phase: if info.has_update {
                        UpdatePhase::Available
                    } else {
                        UpdatePhase::Idle
                    },
                    current_version: current_version.clone(),
                    latest_version: Some(info.latest_version.clone()),
                    progress: None,
                    bytes_per_second: None,
                    error: None,
                })
            };
            emit_update_state(&app, &next_state);
            Ok(info)
        }
        Err(err) => {
            let next_state = {
                let mut guard = state
                    .update_service
                    .lock()
                    .map_err(|_| "Güncelleme servisi kullanılamıyor.".to_string())?;
                guard.set_state(UpdateState {
                    phase: UpdatePhase::Error,
                    current_version: current_version.clone(),
                    latest_version: None,
                    progress: None,
                    bytes_per_second: None,
                    error: Some(err.clone()),
                })
            };
            emit_update_state(&app, &next_state);
            Ok(UpdateInfo {
                has_update: false,
                current_version: current_version.clone(),
                latest_version: current_version,
                release_url: None,
                release_notes: None,
                published_at: None,
                error: Some(err),
            })
        }
    }
}

#[tauri::command]
fn get_update_state(
    state: State<'_, AppState>,
    request: EmptyRequest,
) -> Result<UpdateState, String> {
    let _ = request;
    let guard = state
        .update_service
        .lock()
        .map_err(|_| "Güncelleme servisi kullanılamıyor.".to_string())?;
    Ok(guard.get_state())
}

#[tauri::command]
async fn install_update(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EmptyRequest,
) -> Result<(), String> {
    let _ = request;
    lock_guard_check(&state.lock_service, None)?;
    let info = check_update(app, state, EmptyRequest {}).await?;
    if !info.has_update {
        return Err(info
            .error
            .unwrap_or_else(|| "Yüklenecek yeni sürüm bulunamadı.".to_string()));
    }
    if let Some(url) = info.release_url {
        if !is_trusted_release_url(&url) {
            return Err("Güvenilmeyen güncelleme bağlantısı engellendi.".to_string());
        }
        open_external_url(&url)?;
    }
    Ok(())
}

fn open_external_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Bağlantı açılamadı: {e}"))?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Bağlantı açılamadı: {e}"))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Bağlantı açılamadı: {e}"))?;
        return Ok(());
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn notify_app_ready(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if !state.window_ready.swap(true, Ordering::SeqCst) && !state.suppress_initial_show {
        show_main_window(&app);
    }
    Ok(())
}

fn setup_tray(app: &AppHandle, is_quitting: Arc<AtomicBool>) -> Result<(), String> {
    let show_item = MenuItemBuilder::with_id("show", "Authly'yi Göster")
        .build(app)
        .map_err(|e| format!("Tepsi menü öğesi oluşturulamadı: {e}"))?;
    let quit_item = MenuItemBuilder::with_id("quit", "Çıkış")
        .build(app)
        .map_err(|e| format!("Tepsi çıkış öğesi oluşturulamadı: {e}"))?;
    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .separator()
        .item(&quit_item)
        .build()
        .map_err(|e| format!("Tepsi menüsü oluşturulamadı: {e}"))?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
        .map_err(|e| format!("Tepsi ikonu yüklenemedi: {e}"))?;

    let quit_flag = Arc::clone(&is_quitting);
    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("Authly — Güvenli Authenticator")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app_handle, event| match event.id().as_ref() {
            "show" => show_main_window(app_handle),
            "quit" => {
                quit_flag.store(true, Ordering::SeqCst);
                app_handle.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let should_show = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );
            if should_show {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)
        .map_err(|e| format!("Sistem tepsisi oluşturulamadı: {e}"))?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let is_smoke = std::env::var("AUTHAPP_SMOKE_TEST").as_deref() == Ok("1");

    tauri::Builder::default()
        .setup(move |app| {
            let (db, legacy_dir) = if is_smoke {
                (LocalDatabase::open_in_memory()?, None)
            } else {
                let data_dir = app.path().app_data_dir()?;
                let legacy = data_dir.parent().map(|parent| parent.join("authapp"));
                (LocalDatabase::open(&data_dir.join("authapp.db"))?, legacy)
            };

            let vault: Arc<dyn VaultService> = if is_smoke {
                Arc::new(InMemoryAesGcmVault::new())
            } else {
                Arc::new(KeyringAesGcmVault::new())
            };

            if let Some(legacy_dir) = legacy_dir {
                let _ = migrate_legacy_electron_db(&legacy_dir, &db, vault.as_ref());
            }

            let totp_service = TotpService;
            let account_service = AccountService::new(
                AccountRepository::new(db.connection()),
                vault,
                totp_service,
                Some(GroupRepository::new(db.connection())),
            );
            let settings_service = SettingsService::new(SettingsRepository::new(db.connection()));
            let backup_service = BackupService::new(account_service.clone());
            let lock_service = Arc::new(Mutex::new(LockService::new(SettingsRepository::new(
                db.connection(),
            ))?));
            let update_service = Arc::new(Mutex::new(UpdateService::new(
                app.package_info().version.to_string(),
                !cfg!(debug_assertions),
            )));
            let is_quitting = Arc::new(AtomicBool::new(false));

            let start_minimized = settings_service
                .get_settings()
                .map(|s| s.start_minimized)
                .unwrap_or(false);
            let suppress_initial_show = start_minimized || is_smoke;
            let window_ready = Arc::new(AtomicBool::new(suppress_initial_show));

            let timer_accounts = account_service.clone();
            app.manage(AppState {
                account_service,
                totp_service,
                backup_service,
                settings_service: settings_service.clone(),
                lock_service: Arc::clone(&lock_service),
                update_service,
                is_quitting: Arc::clone(&is_quitting),
                window_ready: Arc::clone(&window_ready),
                suppress_initial_show,
            });

            if !is_smoke {
                let _ = setup_tray(app.handle(), Arc::clone(&is_quitting));
            }

            if let Some(main_window) = app.get_webview_window("main") {
                if suppress_initial_show {
                    let _ = main_window.hide();
                } else {
                    let app_handle_fallback = app.handle().clone();
                    let ready_fallback = Arc::clone(&window_ready);
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(800));
                        if !ready_fallback.swap(true, Ordering::SeqCst) {
                            show_main_window(&app_handle_fallback);
                        }
                    });
                }

                let win_clone = main_window.clone();
                let quit_flag = Arc::clone(&is_quitting);
                let settings_clone = settings_service;
                main_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        if !quit_flag.load(Ordering::SeqCst)
                            && !is_smoke
                            && settings_clone
                                .get_settings()
                                .map(|s| s.close_to_tray)
                                .unwrap_or(true)
                        {
                            api.prevent_close();
                            let _ = win_clone.hide();
                        }
                    }
                });
            }

            let app_handle = app.handle().clone();
            let timer_lock = Arc::clone(&lock_service);
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let maybe_status = {
                    if let Ok(mut guard) = timer_lock.lock() {
                        if guard.check_auto_lock().unwrap_or(false) {
                            guard.snapshot().ok()
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some(status) = maybe_status {
                    timer_accounts.clear_vault_cache();
                    emit_lock_status(&app_handle, &status);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_runtime_info,
            list_accounts,
            create_account,
            import_accounts,
            update_account,
            delete_account,
            get_totp_codes,
            parse_otp_auth_uri,
            copy_totp,
            export_backup,
            import_backup,
            get_settings,
            update_settings,
            get_lock_status,
            verify_pin,
            set_pin,
            remove_pin,
            lock_app,
            record_activity,
            check_update,
            get_update_state,
            install_update,
            notify_app_ready,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
