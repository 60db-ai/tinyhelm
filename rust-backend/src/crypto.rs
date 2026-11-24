//! Laravel-compatible encryption for environment-variable values.
//!
//! Matches Illuminate\Encryption\Encrypter (AES-256-CBC): PKCS7 padding, HMAC-SHA256 over
//! base64(iv)+base64(ciphertext), payload = base64(json{iv,value,mac,tag}). Laravel's `encrypted`
//! cast PHP-serializes the value before encrypting, so we (de)serialize the `s:LEN:"..";` wrapper.

use aes::Aes256;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;
type HmacSha256 = Hmac<Sha256>;

/// Parse APP_KEY ("base64:...." or 32 raw bytes) into a 32-byte key.
pub fn parse_key(app_key: &str) -> Option<[u8; 32]> {
    let raw = match app_key.strip_prefix("base64:") {
        Some(b) => B64.decode(b.trim()).ok()?,
        None => app_key.as_bytes().to_vec(),
    };
    if raw.len() == 32 {
        let mut k = [0u8; 32];
        k.copy_from_slice(&raw);
        Some(k)
    } else {
        None
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Payload {
    iv: String,
    value: String,
    mac: String,
    #[serde(default)]
    tag: String,
}

pub fn decrypt(payload_b64: &str, key: &[u8; 32]) -> Option<String> {
    let json = B64.decode(payload_b64.trim()).ok()?;
    let p: Payload = serde_json::from_slice(&json).ok()?;

    // Verify MAC = hmac_sha256(key, iv_b64 ++ value_b64).
    let mut mac = HmacSha256::new_from_slice(key).ok()?;
    mac.update(p.iv.as_bytes());
    mac.update(p.value.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    if !constant_eq(expected.as_bytes(), p.mac.as_bytes()) {
        return None;
    }

    let iv = B64.decode(&p.iv).ok()?;
    let ct = B64.decode(&p.value).ok()?;
    if iv.len() != 16 {
        return None;
    }
    let dec = Aes256CbcDec::new_from_slices(key, &iv).ok()?;
    let mut buf = ct;
    let pt = dec.decrypt_padded_mut::<Pkcs7>(&mut buf).ok()?;
    let serialized = String::from_utf8(pt.to_vec()).ok()?;
    Some(php_unserialize_string(&serialized).unwrap_or(serialized))
}

/// Laravel `Crypt::encrypt($value)` (PHP-serialized string payload).
pub fn encrypt(plaintext: &str, key: &[u8; 32]) -> Option<String> {
    encrypt_string(&php_serialize_string(plaintext), key)
}

/// Laravel `Crypt::encryptString($value)`: no serialization. Used by the `encrypted` and
/// `encrypted:array` Eloquent casts.
pub fn encrypt_string(serialized: &str, key: &[u8; 32]) -> Option<String> {
    let mut iv = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut iv);
    let enc = Aes256CbcEnc::new_from_slices(key, &iv).ok()?;
    let ct = enc.encrypt_padded_vec_mut::<Pkcs7>(serialized.as_bytes());
    let iv_b64 = B64.encode(iv);
    let value_b64 = B64.encode(&ct);
    let mut mac = HmacSha256::new_from_slice(key).ok()?;
    mac.update(iv_b64.as_bytes());
    mac.update(value_b64.as_bytes());
    let mac_hex = hex::encode(mac.finalize().into_bytes());
    let payload = Payload {
        iv: iv_b64,
        value: value_b64,
        mac: mac_hex,
        tag: String::new(),
    };
    Some(B64.encode(serde_json::to_vec(&payload).ok()?))
}

pub fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut d = 0u8;
    for i in 0..a.len() {
        d |= a[i] ^ b[i];
    }
    d == 0
}

fn php_serialize_string(s: &str) -> String {
    format!("s:{}:\"{}\";", s.len(), s)
}

fn php_unserialize_string(s: &str) -> Option<String> {
    let rest = s.strip_prefix("s:")?;
    let colon = rest.find(':')?;
    let len: usize = rest[..colon].parse().ok()?;
    let body = rest[colon + 1..].strip_prefix('"')?;
    let val = body.get(..len)?;
    if !body[len..].starts_with("\";") {
        return None;
    }
    Some(val.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_laravel_vector() {
        let Ok(k) = std::env::var("TEST_APP_KEY") else {
            return;
        };
        let key = parse_key(&k).expect("key");
        // Rust encrypt -> Rust decrypt
        let enc = encrypt("hello-world", &key).unwrap();
        assert_eq!(decrypt(&enc, &key).unwrap(), "hello-world");
        // Laravel-produced vector -> Rust decrypt (parity)
        if let Ok(v) = std::env::var("TEST_LARAVEL_VEC") {
            assert_eq!(decrypt(&v, &key).unwrap(), "hello-world");
        }
    }
}
