use std::fmt;

// Secret loaded from signing_secret.rs (gitignored — never committed).
// CI generates that file from the VASSL_SIGNING_SECRET GitHub Secret before building.
include!("signing_secret.rs");

// Expiry epoch — days are counted from this date (supports ~45 years forward).
const EPOCH: (i32, u32, u32) = (2020, 1, 1);

// Sentinel value in the 3-byte day field meaning "this key never expires".
const NEVER_EXPIRES: u32 = 0x00FF_FFFF;

// HMAC bytes included in each key — 8 bytes = 64-bit MAC, sufficient for offline validation.
const HMAC_BYTES: usize = 8;

// ── Edition ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Edition {
    Alpha,
    Beta,
    Pro,
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alpha => write!(f, "Alpha"),
            Self::Beta => write!(f, "Beta"),
            Self::Pro => write!(f, "Pro"),
        }
    }
}

impl Edition {
    fn to_byte(&self) -> u8 {
        match self {
            Self::Alpha => 0,
            Self::Beta => 1,
            Self::Pro => 2,
        }
    }

    fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Alpha),
            1 => Some(Self::Beta),
            2 => Some(Self::Pro),
            _ => None,
        }
    }
}

// ── Key validation ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct LicenseInfo {
    pub edition: Edition,
    /// None = key never expires.
    pub expiry: Option<chrono::NaiveDate>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LicenseError {
    InvalidFormat,
    InvalidSignature,
    Expired,
    SecretNotSet,
}

impl fmt::Display for LicenseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "Invalid license key format."),
            Self::InvalidSignature => write!(f, "License key is not valid for this application."),
            Self::Expired => write!(f, "This license has expired."),
            Self::SecretNotSet => write!(f, "Application signing secret has not been configured."),
        }
    }
}

/// Payload layout (4 bytes, big-endian):
///   byte 0    — edition (0=Alpha, 1=Beta, 2=Pro)
///   bytes 1-3 — days since EPOCH as u24; NEVER_EXPIRES (0xFFFFFF) = no expiry
fn encode_payload(edition: &Edition, expiry: Option<chrono::NaiveDate>) -> [u8; 4] {
    let days = match expiry {
        None => NEVER_EXPIRES,
        Some(d) => {
            let epoch = chrono::NaiveDate::from_ymd_opt(EPOCH.0, EPOCH.1, EPOCH.2).unwrap();
            (d - epoch).num_days() as u32
        }
    };
    [
        edition.to_byte(),
        (days >> 16) as u8,
        (days >> 8) as u8,
        days as u8,
    ]
}

fn decode_payload(p: &[u8]) -> Option<(Edition, Option<chrono::NaiveDate>)> {
    if p.len() < 4 {
        return None;
    }
    let edition = Edition::from_byte(p[0])?;
    let days = ((p[1] as u32) << 16) | ((p[2] as u32) << 8) | (p[3] as u32);
    let expiry = if days == NEVER_EXPIRES {
        None
    } else {
        let epoch = chrono::NaiveDate::from_ymd_opt(EPOCH.0, EPOCH.1, EPOCH.2)?;
        Some(epoch + chrono::Duration::days(days as i64))
    };
    Some((edition, expiry))
}

/// Validate a `VASSL-XXXXX-XXXXX-XXXXX-XXXXX` key.
pub fn validate_key(raw: &str) -> Result<LicenseInfo, LicenseError> {
    if SIGNING_SECRET.iter().all(|&b| b == 0) {
        return Err(LicenseError::SecretNotSet);
    }

    let stripped = raw.trim().to_uppercase();
    let stripped = stripped.strip_prefix("VASSL-").unwrap_or(&stripped);
    let compact: String = stripped.chars().filter(|&c| c != '-').collect();

    let bytes = base32_decode(&compact).ok_or(LicenseError::InvalidFormat)?;
    // Expected: 4-byte payload + HMAC_BYTES
    if bytes.len() < 4 + HMAC_BYTES {
        return Err(LicenseError::InvalidFormat);
    }

    let (payload, sig) = bytes.split_at(4);
    let sig = &sig[..HMAC_BYTES];

    let expected = hmac_sha256(payload, &SIGNING_SECRET);
    if !constant_time_eq(&expected[..HMAC_BYTES], sig) {
        return Err(LicenseError::InvalidSignature);
    }

    let (edition, expiry) = decode_payload(payload).ok_or(LicenseError::InvalidFormat)?;

    if let Some(exp) = expiry {
        if chrono::Utc::now().date_naive() > exp {
            return Err(LicenseError::Expired);
        }
    }

    Ok(LicenseInfo { edition, expiry })
}

/// Generate a `VASSL-XXXXX-XXXXX-XXXXX-XXXXX` key (offline keygen tool only).
/// Pass `None` for `expiry` to produce a key that never expires.
#[allow(dead_code)]
pub fn generate_key(edition: Edition, expiry: Option<chrono::NaiveDate>) -> String {
    let payload = encode_payload(&edition, expiry);
    let full_mac = hmac_sha256(&payload, &SIGNING_SECRET);
    let mut all = payload.to_vec();
    all.extend_from_slice(&full_mac[..HMAC_BYTES]);

    let encoded = base32_encode(&all);
    // Format as VASSL-XXXXX-XXXXX-XXXXX-XXXXX (groups of 5)
    let chunks: Vec<String> = encoded
        .chars()
        .collect::<Vec<_>>()
        .chunks(5)
        .map(|c| c.iter().collect())
        .collect();
    format!("VASSL-{}", chunks.join("-"))
}

// ── Build expiry ──────────────────────────────────────────────────────────────

/// Returns true if this is a timed build that has passed its expiry.
pub fn build_expired() -> bool {
    let Some(date_str) = option_env!("VASSL_BUILD_EXPIRES") else {
        return false;
    };
    let Ok(expiry) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
        return false;
    };
    chrono::Utc::now().date_naive() > expiry
}

#[allow(dead_code)]
pub fn build_expiry_date() -> Option<chrono::NaiveDate> {
    let date_str = option_env!("VASSL_BUILD_EXPIRES")?;
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
}

// ── Primitives ────────────────────────────────────────────────────────────────

fn hmac_sha256(data: &[u8], key: &[u8]) -> Vec<u8> {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() <= BLOCK {
        k[..key.len()].copy_from_slice(key);
    } else {
        let h = sha256(key);
        k[..32].copy_from_slice(&h);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = ipad.to_vec();
    inner.extend_from_slice(data);
    let inner_hash = sha256(&inner);
    let mut outer = opad.to_vec();
    outer.extend_from_slice(&inner_hash);
    sha256(&outer).to_vec()
}

fn sha256(data: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(data);
    h.finalize().into()
}

const BASE32_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut buf = 0u64;
    let mut bits = 0u32;
    for &byte in data {
        buf = (buf << 8) | byte as u64;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(BASE32_ALPHABET[((buf >> bits) & 0x1f) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(BASE32_ALPHABET[((buf << (5 - bits)) & 0x1f) as usize] as char);
    }
    out
}

fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let mut buf = 0u64;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for ch in s.chars() {
        let val = BASE32_ALPHABET
            .iter()
            .position(|&b| b == ch.to_ascii_uppercase() as u8)? as u64;
        buf = (buf << 5) | val;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (&x, &y)| acc | (x ^ y))
        == 0
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_sha256_produces_32_bytes() {
        let h = hmac_sha256(b"hello", b"key");
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn base32_roundtrip() {
        let data = b"Hello, World!";
        let encoded = base32_encode(data);
        let decoded = base32_decode(&encoded).unwrap();
        assert_eq!(&decoded[..data.len()], data);
    }

    #[test]
    fn generated_key_has_correct_format() {
        let expiry = chrono::NaiveDate::from_ymd_opt(2099, 12, 31).unwrap();
        let key = generate_key(Edition::Pro, Some(expiry));
        assert!(key.starts_with("VASSL-"));
        let groups: Vec<&str> = key.strip_prefix("VASSL-").unwrap().split('-').collect();
        assert_eq!(groups.len(), 4, "key should have 4 groups after VASSL-");
        assert!(
            groups.iter().all(|g| g.len() == 5),
            "each group should be 5 chars"
        );
    }

    #[test]
    fn generated_key_validates() {
        let expiry = chrono::NaiveDate::from_ymd_opt(2099, 12, 31).unwrap();
        let key = generate_key(Edition::Alpha, Some(expiry));
        let info = validate_key(&key).expect("freshly generated key should validate");
        assert_eq!(info.edition, Edition::Alpha);
        assert_eq!(info.expiry, Some(expiry));
    }

    #[test]
    fn never_expires_key_validates() {
        let key = generate_key(Edition::Pro, None);
        let info = validate_key(&key).expect("never-expires key should validate");
        assert_eq!(info.edition, Edition::Pro);
        assert!(info.expiry.is_none());
    }

    #[test]
    fn expired_key_rejected() {
        let past = chrono::NaiveDate::from_ymd_opt(2020, 1, 2).unwrap();
        let key = generate_key(Edition::Beta, Some(past));
        assert_eq!(validate_key(&key).unwrap_err(), LicenseError::Expired);
    }

    #[test]
    fn tampered_key_rejected() {
        let expiry = chrono::NaiveDate::from_ymd_opt(2099, 12, 31).unwrap();
        let mut key = generate_key(Edition::Pro, Some(expiry));
        let idx = key.find(|c: char| c.is_ascii_alphanumeric()).unwrap();
        let ch = key.chars().nth(idx).unwrap();
        let bad = if ch == 'A' { 'B' } else { 'A' };
        key.replace_range(idx..idx + 1, &bad.to_string());
        assert_eq!(
            validate_key(&key).unwrap_err(),
            LicenseError::InvalidSignature
        );
    }

    #[test]
    fn build_expired_false_when_env_unset() {
        assert!(!build_expired());
    }
}
