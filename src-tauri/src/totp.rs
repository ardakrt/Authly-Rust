use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use url::Url;
use zeroize::Zeroize;

use crate::models::{CreateAccountRequest, TotpAlgorithm};

const BASE32_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpValue {
    pub code: String,
    pub remaining: u32,
    pub period: u32,
}

pub fn normalize_base32_secret(raw: &str) -> Result<String, String> {
    if raw.len() > 4096 {
        return Err("Geçerli bir Base32 kurulum anahtarı girin.".to_string());
    }
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();

    let eq_pos = cleaned.find('=');
    let (unpadded, padding) = match eq_pos {
        Some(idx) => (&cleaned[..idx], &cleaned[idx..]),
        None => (cleaned.as_str(), ""),
    };

    if unpadded.len() < 8
        || padding.len() > 6
        || !padding.chars().all(|c| c == '=')
        || !unpadded
            .chars()
            .all(|c| BASE32_ALPHABET.contains(c))
    {
        return Err("Geçerli bir Base32 kurulum anahtarı girin.".to_string());
    }

    let remainder = unpadded.len() % 8;
    if !matches!(remainder, 0 | 2 | 4 | 5 | 7) {
        return Err("Geçerli bir Base32 kurulum anahtarı girin.".to_string());
    }

    if !padding.is_empty() && cleaned.len() != unpadded.len().div_ceil(8) * 8 {
        return Err("Geçerli bir Base32 kurulum anahtarı girin.".to_string());
    }

    let unused_bits = (unpadded.len() * 5) % 8;
    if unused_bits > 0 {
        let last_char = unpadded
            .chars()
            .last()
            .ok_or_else(|| "Geçerli bir Base32 kurulum anahtarı girin.".to_string())?;
        let last_idx = BASE32_ALPHABET
            .find(last_char)
            .ok_or_else(|| "Geçerli bir Base32 kurulum anahtarı girin.".to_string())?;
        if (last_idx & ((1usize << unused_bits) - 1)) != 0 {
            return Err("Geçerli bir Base32 kurulum anahtarı girin.".to_string());
        }
    }

    Ok(unpadded.to_string())
}

fn compute_hmac(algorithm: TotpAlgorithm, key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    match algorithm {
        TotpAlgorithm::SHA1 => {
            let mut mac =
                Hmac::<Sha1>::new_from_slice(key).map_err(|_| "HMAC anahtarı geçersiz.".to_string())?;
            mac.update(data);
            Ok(mac.finalize().into_bytes().to_vec())
        }
        TotpAlgorithm::SHA256 => {
            let mut mac = Hmac::<Sha256>::new_from_slice(key)
                .map_err(|_| "HMAC anahtarı geçersiz.".to_string())?;
            mac.update(data);
            Ok(mac.finalize().into_bytes().to_vec())
        }
        TotpAlgorithm::SHA512 => {
            let mut mac = Hmac::<Sha512>::new_from_slice(key)
                .map_err(|_| "HMAC anahtarı geçersiz.".to_string())?;
            mac.update(data);
            Ok(mac.finalize().into_bytes().to_vec())
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct TotpService;

impl TotpService {
    pub fn generate(
        &self,
        secret: &str,
        algorithm: TotpAlgorithm,
        digits: u32,
        period: u32,
        timestamp_ms: u64,
    ) -> Result<TotpValue, String> {
        if !(6..=10).contains(&digits) {
            return Err("Basamak sayısı 6 ile 10 arasında olmalıdır.".to_string());
        }
        if !(5..=300).contains(&period) {
            return Err("Süre 5 ile 300 saniye arasında olmalıdır.".to_string());
        }

        let mut normalized = normalize_base32_secret(secret)?;
        let decoded = BASE32_NOPAD
            .decode(normalized.as_bytes())
            .map_err(|_| "Base32 anahtarı çözülemedi.".to_string());
        normalized.zeroize();
        let mut secret_bytes = decoded?;

        let counter = (timestamp_ms / 1000) / u64::from(period);
        let hash_res = compute_hmac(algorithm, &secret_bytes, &counter.to_be_bytes());
        secret_bytes.zeroize();
        let mut hash = hash_res?;

        let offset = (hash[hash.len() - 1] & 0x0f) as usize;
        let bin_code = (u32::from(hash[offset] & 0x7f) << 24)
            | (u32::from(hash[offset + 1]) << 16)
            | (u32::from(hash[offset + 2]) << 8)
            | u32::from(hash[offset + 3]);
        hash.zeroize();

        let modulo = 10u64.pow(digits);
        let otp = u64::from(bin_code) % modulo;
        let code = format!("{otp:0width$}", width = digits as usize);

        let period_ms = u64::from(period) * 1000;
        let rem_ms = period_ms - (timestamp_ms % period_ms);
        let remaining = (rem_ms.div_ceil(1000)).max(1) as u32;

        Ok(TotpValue {
            code,
            remaining,
            period,
        })
    }

    pub fn parse_uri(&self, uri: &str) -> Result<CreateAccountRequest, String> {
        let parsed = Url::parse(uri).map_err(|_| "otpauth bağlantısı okunamadı.".to_string())?;
        if parsed.scheme() != "otpauth" || parsed.host_str() != Some("totp") {
            return Err("Only TOTP URIs are supported.".to_string());
        }

        let raw_label = parsed.path().trim_start_matches('/');
        let decoded_label = url_decode(raw_label)?;
        let (label_issuer, account_name) = match decoded_label.split_once(':') {
            Some((iss, acc)) => (iss.trim().to_string(), acc.trim().to_string()),
            None => (String::new(), decoded_label.trim().to_string()),
        };

        let mut secret: Option<String> = None;
        let mut query_issuer: Option<String> = None;
        let mut algorithm = TotpAlgorithm::SHA1;
        let mut digits = 6u32;
        let mut period = 30u32;

        for (key, value) in parsed.query_pairs() {
            match key.as_ref() {
                "secret" => secret = Some(value.into_owned()),
                "issuer" => query_issuer = Some(value.trim().to_string()),
                "algorithm" => algorithm = TotpAlgorithm::from_str_strict(&value)?,
                "digits" => {
                    digits = value
                        .parse::<u32>()
                        .map_err(|_| "Geçersiz TOTP basamak sayısı.".to_string())?;
                }
                "period" => {
                    period = value
                        .parse::<u32>()
                        .map_err(|_| "Geçersiz TOTP süresi.".to_string())?;
                }
                _ => {}
            }
        }

        let secret = normalize_base32_secret(
            secret
                .as_deref()
                .ok_or_else(|| "otpauth bağlantısında gizli anahtar eksik.".to_string())?,
        )?;
        let issuer = query_issuer
            .filter(|s| !s.is_empty())
            .or_else(|| {
                if label_issuer.is_empty() {
                    None
                } else {
                    Some(label_issuer)
                }
            })
            .unwrap_or_else(|| "Authenticator".to_string());

        let account_name = if account_name.is_empty() {
            issuer.clone()
        } else {
            account_name
        };

        if issuer.chars().count() > 120 || account_name.chars().count() > 240 {
            return Err("Hesap veya servis adı izin verilen uzunluğu aşıyor.".to_string());
        }
        if !(6..=10).contains(&digits) || !(5..=300).contains(&period) {
            return Err("Geçersiz TOTP parametreleri.".to_string());
        }

        Ok(CreateAccountRequest {
            issuer,
            account_name,
            secret,
            algorithm,
            digits,
            period,
            favorite: false,
            group_id: None,
        })
    }
}

fn url_decode(input: &str) -> Result<String, String> {
    let mut bytes = Vec::with_capacity(input.len());
    let raw = input.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            if i + 2 >= raw.len() {
                return Err("otpauth bağlantısı okunamadı.".to_string());
            }
            let hex_str = std::str::from_utf8(&raw[i + 1..i + 3])
                .map_err(|_| "otpauth bağlantısı okunamadı.".to_string())?;
            let byte = u8::from_str_radix(hex_str, 16)
                .map_err(|_| "otpauth bağlantısı okunamadı.".to_string())?;
            bytes.push(byte);
            i += 3;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    String::from_utf8(bytes).map_err(|_| "otpauth bağlantısı okunamadı.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_rfc_6238_vectors() {
        let service = TotpService;
        let vectors = [
            (
                TotpAlgorithm::SHA1,
                "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
                "94287082",
            ),
            (
                TotpAlgorithm::SHA256,
                "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA",
                "46119246",
            ),
            (
                TotpAlgorithm::SHA512,
                "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNA",
                "90693936",
            ),
        ];

        for (algorithm, secret, expected) in vectors {
            let value = service.generate(secret, algorithm, 8, 30, 59_000).unwrap();
            assert_eq!(value.code, expected);
        }
    }

    #[test]
    fn parses_standard_otpauth_totp_uri() {
        let service = TotpService;
        let parsed = service
            .parse_uri("otpauth://totp/Example:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Example")
            .unwrap();
        assert_eq!(parsed.issuer, "Example");
        assert_eq!(parsed.account_name, "alice@example.com");
        assert_eq!(parsed.secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(parsed.algorithm, TotpAlgorithm::SHA1);
        assert_eq!(parsed.digits, 6);
        assert_eq!(parsed.period, 30);
    }

    #[test]
    fn rejects_malformed_base32_and_normalizes_valid() {
        for bad in [
            "INVALID!",
            "========",
            "AAAAAAAAA",
            "AAAAAAAAAB",
            "AAAAAAAAAA=",
            "AAAAAAAA=",
        ] {
            assert!(normalize_base32_secret(bad).is_err(), "expected err for {bad}");
        }
        assert_eq!(
            normalize_base32_secret(" jbsw-y3dp ehpk3pxp ").unwrap(),
            "JBSWY3DPEHPK3PXP"
        );
        assert_eq!(
            normalize_base32_secret("AAAAAAAAAA======").unwrap(),
            "AAAAAAAAAA"
        );
    }

    #[test]
    fn validates_uri_issuer_and_account_name_by_char_count() {
        let service = TotpService;
        // Multibyte UTF-8 characters exceed byte length limits while staying within char count limits.
        let exact_issuer = "ş".repeat(120);
        let exact_account = "ğ".repeat(240);
        let valid_uri = format!(
            "otpauth://totp/{exact_issuer}:{exact_account}?secret=JBSWY3DPEHPK3PXP&issuer={exact_issuer}"
        );
        let parsed = service.parse_uri(&valid_uri).unwrap();
        assert_eq!(parsed.issuer, exact_issuer);
        assert_eq!(parsed.account_name, exact_account);

        let over_issuer = "ş".repeat(121);
        let over_issuer_uri = format!(
            "otpauth://totp/{over_issuer}:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer={over_issuer}"
        );
        assert!(service.parse_uri(&over_issuer_uri).is_err());

        let over_account = "ğ".repeat(241);
        let over_account_uri = format!(
            "otpauth://totp/Example:{over_account}?secret=JBSWY3DPEHPK3PXP&issuer=Example"
        );
        assert!(service.parse_uri(&over_account_uri).is_err());
    }
}
