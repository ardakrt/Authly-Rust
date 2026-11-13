use crate::database::SettingsRepository;
use crate::models::{AppSettings, LanguagePreference, ThemePreference, UpdateSettingsRequest};

#[derive(Clone)]
pub struct SettingsService {
    repository: SettingsRepository,
}

impl SettingsService {
    pub fn new(repository: SettingsRepository) -> Self {
        Self { repository }
    }

    pub fn get_settings(&self) -> Result<AppSettings, String> {
        let theme_raw = self
            .repository
            .get("theme")?
            .unwrap_or_else(|| "system".to_string());
        let language_raw = self
            .repository
            .get("language")?
            .unwrap_or_else(|| "tr".to_string());
        let close_to_tray_raw = self
            .repository
            .get("closeToTray")?
            .unwrap_or_else(|| "true".to_string());
        let start_minimized_raw = self
            .repository
            .get("startMinimized")?
            .unwrap_or_else(|| "false".to_string());

        let theme = match theme_raw.as_str() {
            "light" => ThemePreference::Light,
            "dark" => ThemePreference::Dark,
            _ => ThemePreference::System,
        };
        let language = if language_raw == "en" {
            LanguagePreference::En
        } else {
            LanguagePreference::Tr
        };
        let close_to_tray = close_to_tray_raw != "false";
        let start_minimized = start_minimized_raw == "true";

        Ok(AppSettings {
            theme,
            language,
            close_to_tray,
            start_minimized,
        })
    }

    pub fn update_settings(&self, input: UpdateSettingsRequest) -> Result<AppSettings, String> {
        if let Some(theme) = input.theme {
            let val = match theme {
                ThemePreference::System => "system",
                ThemePreference::Light => "light",
                ThemePreference::Dark => "dark",
            };
            self.repository.set("theme", val)?;
        }
        if let Some(language) = input.language {
            let val = match language {
                LanguagePreference::Tr => "tr",
                LanguagePreference::En => "en",
            };
            self.repository.set("language", val)?;
        }
        if let Some(close_to_tray) = input.close_to_tray {
            self.repository
                .set("closeToTray", if close_to_tray { "true" } else { "false" })?;
        }
        if let Some(start_minimized) = input.start_minimized {
            self.repository.set(
                "startMinimized",
                if start_minimized { "true" } else { "false" },
            )?;
        }
        self.get_settings()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::LocalDatabase;

    #[test]
    fn returns_defaults_and_persists_updates() {
        let db = LocalDatabase::open_in_memory().unwrap();
        let service = SettingsService::new(SettingsRepository::new(db.connection()));

        assert_eq!(
            service.get_settings().unwrap(),
            AppSettings {
                theme: ThemePreference::System,
                language: LanguagePreference::Tr,
                close_to_tray: true,
                start_minimized: false,
            }
        );

        let updated = service
            .update_settings(UpdateSettingsRequest {
                theme: Some(ThemePreference::Dark),
                language: Some(LanguagePreference::En),
                close_to_tray: Some(false),
                start_minimized: None,
            })
            .unwrap();

        assert_eq!(
            updated,
            AppSettings {
                theme: ThemePreference::Dark,
                language: LanguagePreference::En,
                close_to_tray: false,
                start_minimized: false,
            }
        );
    }
}
