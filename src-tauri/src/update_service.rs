use reqwest::StatusCode;
use serde::Deserialize;

use crate::models::{UpdateInfo, UpdatePhase, UpdateState};

pub const GITHUB_RELEASES_API_URL: &str =
    "https://api.github.com/repos/ardakrt/Authly-Rust/releases/latest";
pub const GITHUB_RELEASES_BASE_URL: &str = "https://github.com/ardakrt/Authly-Rust/releases";

pub fn is_trusted_release_url(url: &str) -> bool {
    if url.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
        return false;
    }
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return false;
    }
    let lower_path = parsed.path().to_ascii_lowercase();
    lower_path == "/ardakrt/authly-rust/releases"
        || lower_path.starts_with("/ardakrt/authly-rust/releases/")
}

#[derive(Debug, Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    html_url: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
}

pub struct UpdateService {
    current_version: String,
    is_packaged: bool,
    state: UpdateState,
}

impl UpdateService {
    pub fn new(current_version: String, is_packaged: bool) -> Self {
        let state = UpdateState {
            phase: UpdatePhase::Idle,
            current_version: current_version.clone(),
            latest_version: None,
            progress: None,
            bytes_per_second: None,
            error: None,
        };
        Self {
            current_version,
            is_packaged,
            state,
        }
    }

    pub fn get_state(&self) -> UpdateState {
        self.state.clone()
    }

    pub fn set_state(&mut self, state: UpdateState) -> UpdateState {
        self.state = state;
        self.state.clone()
    }

    pub fn is_packaged(&self) -> bool {
        self.is_packaged
    }

    pub fn current_version(&self) -> &str {
        &self.current_version
    }

    pub fn is_newer_version(latest: &str, current: &str) -> bool {
        let parse = |v: &str| -> Vec<u64> {
            v.trim_start_matches('v')
                .split('.')
                .map(|part| part.parse::<u64>().unwrap_or(0))
                .collect()
        };
        let latest_parts = parse(latest);
        let current_parts = parse(current);
        let max_len = latest_parts.len().max(current_parts.len());

        for idx in 0..max_len {
            let l = latest_parts.get(idx).copied().unwrap_or(0);
            let c = current_parts.get(idx).copied().unwrap_or(0);
            if l > c {
                return true;
            }
            if l < c {
                return false;
            }
        }
        false
    }
}

pub async fn fetch_latest_github_release(current_version: &str) -> Result<UpdateInfo, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("Authly/{current_version}"))
        .build()
        .map_err(|e| format!("HTTP istemcisi başlatılamadı: {e}"))?;

    let response = client
        .get(GITHUB_RELEASES_API_URL)
        .send()
        .await
        .map_err(|e| format!("Güncelleme sunucusuna bağlanılamadı: {e}"))?;

    if response.status() == StatusCode::NOT_FOUND {
        return Ok(UpdateInfo {
            has_update: false,
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            release_url: None,
            release_notes: None,
            published_at: None,
            error: None,
        });
    }

    if !response.status().is_success() {
        return Err(format!(
            "Güncelleme denetimi başarısız (HTTP {}).",
            response.status()
        ));
    }

    let release: GithubReleaseResponse = response
        .json()
        .await
        .map_err(|_| "Güncelleme yanıtı okunamadı.".to_string())?;

    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let has_update = UpdateService::is_newer_version(&latest_version, current_version);
    let fallback_url = format!("{GITHUB_RELEASES_BASE_URL}/tag/v{latest_version}");
    let release_url = if has_update {
        Some(
            release
                .html_url
                .filter(|url| is_trusted_release_url(url))
                .unwrap_or(fallback_url),
        )
    } else {
        None
    };

    Ok(UpdateInfo {
        has_update,
        current_version: current_version.to_string(),
        latest_version,
        release_url,
        release_notes: release.body,
        published_at: release.published_at,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semantic_versions() {
        assert!(UpdateService::is_newer_version("0.2.0", "0.1.0"));
        assert!(UpdateService::is_newer_version("1.0.0", "0.9.9"));
        assert!(!UpdateService::is_newer_version("0.1.0", "0.1.0"));
        assert!(!UpdateService::is_newer_version("0.0.9", "0.1.0"));
        assert!(UpdateService::is_newer_version("v0.4.0", "0.3.12"));
    }

    #[test]
    fn validates_trusted_authly_rust_release_urls() {
        assert!(is_trusted_release_url(
            "https://github.com/ardakrt/Authly-Rust/releases"
        ));
        assert!(is_trusted_release_url(
            "https://github.com/ardakrt/authly-rust/releases/tag/v0.3.13"
        ));
        assert!(!is_trusted_release_url(
            "https://github.com/ardakrt/authly/releases/tag/v0.3.13"
        ));
        assert!(!is_trusted_release_url(
            "https://github.com/ardakrt/Authly-Rust/releases-evil/tag/v1.0.0"
        ));
        assert!(!is_trusted_release_url(
            "http://github.com/ardakrt/Authly-Rust/releases/tag/v0.3.13"
        ));
        assert!(!is_trusted_release_url(
            "https://github.com@evil.com/ardakrt/Authly-Rust/releases/tag/v0.3.13"
        ));
        assert!(!is_trusted_release_url(
            "https://github.com/ardakrt/Authly-Rust/releases/tag/v0.3.13\ncalc"
        ));
    }
}
