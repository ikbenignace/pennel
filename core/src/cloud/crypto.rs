use crate::error::PennelError;
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::{Aead, KeyInit, Payload as AeadPayload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fmt;
use unicode_normalization::UnicodeNormalization;
use zeroize::{Zeroize, Zeroizing};

const FORMAT_V1: u8 = 0x01;
const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;
const KEY_LEN: usize = 32;
const SALT_LEN: usize = 16;
const VAULT_AD: &[u8] = b"pennel-vault-v1";
const ID_KEY_LABEL: &[u8] = b"pennel/object-id/v1";
const ENC_KEY_LABEL: &[u8] = b"pennel/object-enc/v1";

const RECOVERY_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const RECOVERY_SYMBOLS: usize = 24;
const RECOVERY_GROUP: usize = 4;

const KDF_ALG: &str = "argon2id";
// Upper bounds on server-supplied parameters, so a hostile or broken server
// cannot make unwrap allocate gigabytes or spin for minutes.
const MAX_M_KIB: u32 = 262_144;
const MAX_T: u32 = 8;
const MAX_P: u32 = 4;
const MAX_SALT_LEN: usize = 64;

pub struct VaultKey([u8; KEY_LEN]);

impl VaultKey {
    pub fn generate() -> Self {
        let mut bytes = [0u8; KEY_LEN];
        fill_random(&mut bytes);
        Self(bytes)
    }

    pub fn to_b64(&self) -> String {
        B64.encode(self.0)
    }

    pub fn from_b64(encoded: &str) -> Option<Self> {
        let bytes = Zeroizing::new(B64.decode(encoded.trim()).ok()?);
        let array: [u8; KEY_LEN] = bytes.as_slice().try_into().ok()?;
        Some(Self(array))
    }
}

impl Drop for VaultKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for VaultKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VaultKey(..)")
    }
}

pub fn new_recovery_code() -> String {
    let mut bytes = Zeroizing::new([0u8; RECOVERY_SYMBOLS]);
    fill_random(bytes.as_mut());
    // 32 divides 256, so masking to 5 bits keeps every symbol uniform.
    let symbols: Zeroizing<Vec<u8>> = Zeroizing::new(
        bytes
            .iter()
            .map(|b| RECOVERY_ALPHABET[usize::from(b & 0x1f)])
            .collect(),
    );
    group_symbols(&symbols)
}

pub fn normalise_recovery_code(input: &str) -> Option<String> {
    let mut symbols = Zeroizing::new(Vec::with_capacity(RECOVERY_SYMBOLS));
    for c in input.chars() {
        if c == '-' || c.is_whitespace() {
            continue;
        }
        let symbol = match c.to_ascii_uppercase() {
            'O' => b'0',
            'I' | 'L' => b'1',
            other if other.is_ascii() && RECOVERY_ALPHABET.contains(&(other as u8)) => other as u8,
            _ => return None,
        };
        if symbols.len() == RECOVERY_SYMBOLS {
            return None;
        }
        symbols.push(symbol);
    }
    (symbols.len() == RECOVERY_SYMBOLS).then(|| group_symbols(&symbols))
}

fn group_symbols(symbols: &[u8]) -> String {
    let mut code = String::with_capacity(RECOVERY_SYMBOLS + RECOVERY_SYMBOLS / RECOVERY_GROUP);
    for (index, group) in symbols.chunks(RECOVERY_GROUP).enumerate() {
        if index > 0 {
            code.push('-');
        }
        code.extend(group.iter().map(|b| char::from(*b)));
    }
    code
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kdf {
    pub alg: String,
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
    pub salt: String,
    /// [`vault_check`] of the wrapped key, so a device that kept a key can tell whether it
    /// belongs to this vault without the recovery code. Absent on vaults made before it existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_check: Option<String>,
}

#[derive(Clone, Copy)]
struct KdfParams {
    m_kib: u32,
    t: u32,
    p: u32,
}

const PRODUCTION_PARAMS: KdfParams = KdfParams {
    m_kib: 65536,
    t: 3,
    p: 1,
};

#[cfg(not(test))]
const WRAP_PARAMS: KdfParams = PRODUCTION_PARAMS;

// Keeps the unit tests fast; one test still runs PRODUCTION_PARAMS.
#[cfg(test)]
const WRAP_PARAMS: KdfParams = KdfParams {
    m_kib: 64,
    t: 1,
    p: 1,
};

/// Only the canonical form from [`new_recovery_code`] is accepted, so a vault can
/// never be wrapped under a code that `unwrap` would later refuse.
pub fn wrap(key: &VaultKey, code: &str) -> Result<(Vec<u8>, Kdf), PennelError> {
    wrap_with(key, code, WRAP_PARAMS)
}

fn wrap_with(key: &VaultKey, code: &str, params: KdfParams) -> Result<(Vec<u8>, Kdf), PennelError> {
    if normalise_recovery_code(code).as_deref() != Some(code) {
        return Err(wrong_recovery_code());
    }
    let mut salt = [0u8; SALT_LEN];
    fill_random(&mut salt);
    let kdf = Kdf {
        alg: KDF_ALG.to_owned(),
        m_kib: params.m_kib,
        t: params.t,
        p: params.p,
        salt: B64.encode(salt),
        key_check: Some(vault_check(key)),
    };
    let wrapping_key =
        derive_wrapping_key(code.as_bytes(), &salt, params).ok_or_else(vault_invalid)?;
    Ok((encrypt(&wrapping_key, VAULT_AD, &key.0), kdf))
}

pub fn unwrap(wrapped: &[u8], kdf: &Kdf, code: &str) -> Result<VaultKey, PennelError> {
    let (params, salt) = validate_kdf(kdf).ok_or_else(vault_invalid)?;
    if wrapped.len() != 1 + NONCE_LEN + KEY_LEN + TAG_LEN || wrapped[0] != FORMAT_V1 {
        return Err(vault_invalid());
    }
    let code = Zeroizing::new(normalise_recovery_code(code).ok_or_else(wrong_recovery_code)?);
    let wrapping_key =
        derive_wrapping_key(code.as_bytes(), &salt, params).ok_or_else(vault_invalid)?;
    let plaintext = decrypt(&wrapping_key, VAULT_AD, wrapped).ok_or_else(wrong_recovery_code)?;
    let bytes: [u8; KEY_LEN] = plaintext
        .as_slice()
        .try_into()
        .map_err(|_| vault_invalid())?;
    Ok(VaultKey(bytes))
}

fn validate_kdf(kdf: &Kdf) -> Option<(KdfParams, Vec<u8>)> {
    let salt = decode_b64(&kdf.salt)?;
    let valid = kdf.alg == KDF_ALG
        && (SALT_LEN..=MAX_SALT_LEN).contains(&salt.len())
        && (1..=MAX_P).contains(&kdf.p)
        && (1..=MAX_T).contains(&kdf.t)
        && (8 * kdf.p..=MAX_M_KIB).contains(&kdf.m_kib);
    valid.then_some((
        KdfParams {
            m_kib: kdf.m_kib,
            t: kdf.t,
            p: kdf.p,
        },
        salt,
    ))
}

fn derive_wrapping_key(
    code: &[u8],
    salt: &[u8],
    params: KdfParams,
) -> Option<Zeroizing<[u8; KEY_LEN]>> {
    let params = Params::new(params.m_kib, params.t, params.p, Some(KEY_LEN)).ok()?;
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(code, salt, out.as_mut())
        .ok()?;
    Some(out)
}

/// Names a vault key without revealing it, so a device can tell which vault its sync state
/// belongs to.
pub fn vault_check(key: &VaultKey) -> String {
    object_id(key, "pennel/vault-check")
}

pub fn object_id(key: &VaultKey, object_key: &str) -> String {
    let normalised: String = object_key.nfc().collect();
    let digest = hmac_sha256(&subkey(key, ID_KEY_LABEL)[..], normalised.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest.iter() {
        hex.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        hex.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
    }
    hex
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Payload {
    pub key: String,
    pub folder: Option<String>,
    #[serde(with = "content_b64")]
    pub content: Vec<u8>,
    pub modified_ms: i64,
    pub device: String,
}

impl fmt::Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Payload")
            .field("content_len", &self.content.len())
            .field("modified_ms", &self.modified_ms)
            .finish_non_exhaustive()
    }
}

mod content_b64 {
    use super::B64;
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&B64.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        B64.decode(encoded).map_err(serde::de::Error::custom)
    }
}

pub fn seal(key: &VaultKey, object_id: &str, generation: u32, payload: &Payload) -> Vec<u8> {
    let plaintext =
        Zeroizing::new(serde_json::to_vec(payload).expect("payload serialises to JSON"));
    encrypt(&enc_key(key), &object_ad(object_id, generation), &plaintext)
}

pub fn open(
    key: &VaultKey,
    object_id: &str,
    generation: u32,
    blob: &[u8],
) -> Result<Payload, PennelError> {
    let plaintext = decrypt(&enc_key(key), &object_ad(object_id, generation), blob)
        .ok_or_else(object_unreadable)?;
    let payload: Payload = serde_json::from_slice(&plaintext).map_err(|_| object_unreadable())?;
    // A valid blob replayed under another id must not be written to the wrong path.
    if self::object_id(key, &payload.key) != object_id {
        return Err(object_unreadable());
    }
    Ok(payload)
}

fn enc_key(key: &VaultKey) -> Zeroizing<[u8; KEY_LEN]> {
    subkey(key, ENC_KEY_LABEL)
}

// One root key, separate keys per purpose: HMAC-SHA256(root, label).
fn subkey(key: &VaultKey, label: &[u8]) -> Zeroizing<[u8; KEY_LEN]> {
    Zeroizing::new(hmac_sha256(&key.0, label))
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; KEY_LEN] {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(message);
    mac.finalize().into_bytes().into()
}

fn object_ad(object_id: &str, generation: u32) -> Vec<u8> {
    let mut ad = Vec::with_capacity(object_id.len() + 4);
    ad.extend_from_slice(object_id.as_bytes());
    ad.extend_from_slice(&generation.to_be_bytes());
    ad
}

// Random 192-bit nonces: collisions are negligible at any realistic message count,
// so no nonce state has to be kept or synchronised between devices.
fn encrypt(key: &[u8; KEY_LEN], ad: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce);
    let ciphertext = XChaCha20Poly1305::new_from_slice(key)
        .expect("32-byte key")
        .encrypt(
            &XNonce::from(nonce),
            AeadPayload {
                msg: plaintext,
                aad: ad,
            },
        )
        .expect("XChaCha20-Poly1305 encryption is infallible below 256 GiB");
    let mut blob = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
    blob.push(FORMAT_V1);
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ciphertext);
    blob
}

fn decrypt(key: &[u8; KEY_LEN], ad: &[u8], blob: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    let (&version, rest) = blob.split_first()?;
    if version != FORMAT_V1 || rest.len() < NONCE_LEN + TAG_LEN {
        return None;
    }
    let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
    let nonce = XNonce::try_from(nonce).ok()?;
    XChaCha20Poly1305::new_from_slice(key)
        .ok()?
        .decrypt(
            &nonce,
            AeadPayload {
                msg: ciphertext,
                aad: ad,
            },
        )
        .ok()
        .map(Zeroizing::new)
}

fn fill_random(dest: &mut [u8]) {
    // Without OS randomness no key or nonce is safe; stopping beats a weak fallback.
    getrandom::fill(dest).expect("OS random number generator unavailable");
}

fn decode_b64(encoded: &str) -> Option<Vec<u8>> {
    B64.decode(encoded).ok()
}

fn wrong_recovery_code() -> PennelError {
    PennelError::Cloud {
        code: "recovery_code".into(),
        message: "This recovery code is incorrect.".into(),
    }
}

pub(crate) fn vault_invalid() -> PennelError {
    PennelError::Cloud {
        code: "vault_invalid".into(),
        message: "The Pennel Cloud vault cannot be read.".into(),
    }
}

pub(crate) fn object_unreadable() -> PennelError {
    PennelError::Cloud {
        code: "object_unreadable".into(),
        message: "A file from Pennel Cloud could not be decrypted.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: &str = "7K3M-Q9TV-W2XZ-H4JN-P6RS-A8BC";

    fn payload() -> Payload {
        Payload {
            key: "meeting/0192f3a4-0000-7000-8000-000000000001/notes.md".into(),
            folder: Some("2026-10-25 1400 Café overleg".into()),
            content: b"# Notities\n\nGeheim.".to_vec(),
            modified_ms: 1_761_400_000_000,
            device: "MacBook van Ignace".into(),
        }
    }

    fn assert_recovery_code_error(result: Result<VaultKey, PennelError>) {
        match result {
            Err(error) => assert_eq!(
                error,
                PennelError::Cloud {
                    code: "recovery_code".into(),
                    message: "This recovery code is incorrect.".into(),
                }
            ),
            Ok(_) => panic!("expected recovery_code error"),
        }
    }

    #[test]
    fn vault_key_b64_roundtrip_and_redacted_debug() {
        let key = VaultKey::generate();
        let encoded = key.to_b64();
        let decoded = VaultKey::from_b64(&encoded).expect("valid key");
        assert_eq!(decoded.to_b64(), encoded);
        assert_ne!(VaultKey::generate().to_b64(), encoded);
        assert!(VaultKey::from_b64("too-short").is_none());
        assert!(VaultKey::from_b64(&base64_of(&[0u8; 31])).is_none());
        assert!(!format!("{key:?}").contains(&encoded));
    }

    fn base64_of(bytes: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn wrap_unwrap_roundtrip() {
        let key = VaultKey::generate();
        let (wrapped, kdf) = wrap(&key, CODE).unwrap();
        assert_eq!(wrapped[0], 0x01);
        assert_eq!(wrapped.len(), 1 + 24 + 32 + 16);
        assert_eq!(kdf.alg, "argon2id");
        let unwrapped = unwrap(&wrapped, &kdf, CODE).expect("unwraps");
        assert_eq!(unwrapped.to_b64(), key.to_b64());

        let sloppy = "7k3m q9tv w2xz h4jn p6rs a8bc";
        let unwrapped = unwrap(&wrapped, &kdf, sloppy).expect("normalised code unwraps");
        assert_eq!(unwrapped.to_b64(), key.to_b64());
    }

    #[test]
    fn wrap_unwrap_roundtrip_with_production_params() {
        let key = VaultKey::generate();
        let (wrapped, kdf) = wrap_with(&key, CODE, PRODUCTION_PARAMS).unwrap();
        assert_eq!(
            (kdf.alg.as_str(), kdf.m_kib, kdf.t, kdf.p),
            ("argon2id", 65536, 3, 1)
        );
        assert_eq!(decode_b64(&kdf.salt).map(|s| s.len()), Some(16));
        let json = serde_json::to_value(&kdf).unwrap();
        assert_eq!(
            json.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["alg", "key_check", "m_kib", "p", "salt", "t"]
        );
        assert_eq!(kdf.key_check, Some(vault_check(&key)));
        let mut older = json.clone();
        older.as_object_mut().unwrap().remove("key_check");
        let older: Kdf = serde_json::from_value(older).unwrap();
        assert_eq!(older.key_check, None);
        let unwrapped = unwrap(&wrapped, &kdf, CODE).expect("unwraps");
        assert_eq!(unwrapped.to_b64(), key.to_b64());
    }

    #[test]
    fn wrap_uses_fresh_salt_and_nonce() {
        let key = VaultKey::generate();
        let (first, first_kdf) = wrap(&key, CODE).unwrap();
        let (second, second_kdf) = wrap(&key, CODE).unwrap();
        assert_ne!(first_kdf.salt, second_kdf.salt);
        assert_ne!(first[1..25], second[1..25]);
    }

    #[test]
    fn wrong_recovery_code_rejected() {
        let key = VaultKey::generate();
        let (wrapped, kdf) = wrap(&key, CODE).unwrap();
        assert_recovery_code_error(unwrap(&wrapped, &kdf, "7K3M-Q9TV-W2XZ-H4JN-P6RS-A8BD"));
        assert_recovery_code_error(unwrap(&wrapped, &kdf, "not a code"));
        assert_recovery_code_error(unwrap(&wrapped, &kdf, ""));
    }

    #[test]
    fn unwrap_rejects_malformed_vault_without_panicking() {
        let key = VaultKey::generate();
        let (wrapped, kdf) = wrap(&key, CODE).unwrap();
        for blob in [
            &[][..],
            &[0x01][..],
            &wrapped[..40],
            &wrapped[..wrapped.len() - 1],
        ] {
            assert!(unwrap(blob, &kdf, CODE).is_err());
        }
        let mut other_version = wrapped.clone();
        other_version[0] = 0x02;
        assert!(unwrap(&other_version, &kdf, CODE).is_err());

        let bad_kdfs = [
            Kdf {
                alg: "argon2i".into(),
                ..kdf.clone()
            },
            Kdf {
                salt: "!!".into(),
                ..kdf.clone()
            },
            Kdf {
                salt: base64_of(&[1u8; 4]),
                ..kdf.clone()
            },
            Kdf {
                m_kib: 0,
                ..kdf.clone()
            },
            Kdf {
                m_kib: u32::MAX,
                ..kdf.clone()
            },
            Kdf {
                t: 0,
                ..kdf.clone()
            },
            Kdf {
                t: 1000,
                ..kdf.clone()
            },
            Kdf {
                p: 0,
                ..kdf.clone()
            },
            Kdf {
                m_kib: 262_145,
                ..kdf.clone()
            },
            Kdf {
                t: 9,
                ..kdf.clone()
            },
            Kdf {
                p: 5,
                m_kib: 64,
                ..kdf.clone()
            },
        ];
        for bad in bad_kdfs {
            assert!(unwrap(&wrapped, &bad, CODE).is_err());
        }
    }

    #[test]
    fn kdf_bounds_accept_production_and_reject_above_limits() {
        let salt = base64_of(&[9u8; 16]);
        let kdf = |m_kib, t, p| Kdf {
            alg: "argon2id".into(),
            m_kib,
            t,
            p,
            salt: salt.clone(),
            key_check: None,
        };
        assert!(validate_kdf(&kdf(65536, 3, 1)).is_some());
        assert!(validate_kdf(&kdf(262_144, 8, 4)).is_some());
        assert!(validate_kdf(&kdf(262_145, 3, 1)).is_none());
        assert!(validate_kdf(&kdf(65536, 9, 1)).is_none());
        assert!(validate_kdf(&kdf(65536, 3, 5)).is_none());
        assert!(validate_kdf(&kdf(31, 3, 4)).is_none());
    }

    #[test]
    fn wrap_rejects_invalid_or_non_canonical_code() {
        let key = VaultKey::generate();
        for code in [
            "",
            "not a code",
            "7K3M-Q9TV-W2XZ-H4JN-P6RS-A8B",
            "7k3m-q9tv-w2xz-h4jn-p6rs-a8bc",
            "7K3MQ9TVW2XZH4JNP6RSA8BC",
            "7K3M Q9TV W2XZ H4JN P6RS A8BC",
            "OK3M-Q9TV-W2XZ-H4JN-P6RS-A8BC",
        ] {
            match wrap(&key, code) {
                Err(PennelError::Cloud { code, .. }) => assert_eq!(code, "recovery_code"),
                other => panic!("expected recovery_code error, got ok={}", other.is_ok()),
            }
        }
    }

    #[test]
    fn recovery_code_format_and_normalise() {
        const ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        let code = new_recovery_code();
        let groups: Vec<&str> = code.split('-').collect();
        assert_eq!(groups.len(), 6);
        assert!(groups.iter().all(|group| group.len() == 4));
        assert!(
            code.chars()
                .filter(|c| *c != '-')
                .all(|c| ALPHABET.contains(c))
        );
        assert_ne!(new_recovery_code(), code);
        assert_eq!(
            normalise_recovery_code(&code).as_deref(),
            Some(code.as_str())
        );

        assert_eq!(
            normalise_recovery_code("abcd efgh-jkmn pqrs-tvwx yz01").as_deref(),
            Some("ABCD-EFGH-JKMN-PQRS-TVWX-YZ01")
        );
        assert_eq!(
            normalise_recovery_code("o0Oo-iIlL-1111-2222-3333-4444").as_deref(),
            Some("0000-1111-1111-2222-3333-4444")
        );
        assert_eq!(
            normalise_recovery_code("ABCD-EFGH-JKMN-PQRS-TVWX-YZ0"),
            None
        );
        assert_eq!(
            normalise_recovery_code("ABCD-EFGH-JKMN-PQRS-TVWX-YZ012"),
            None
        );
        assert_eq!(
            normalise_recovery_code("ABCD-EFGH-JKMN-PQRS-TVWX-YZ0U"),
            None
        );
        assert_eq!(
            normalise_recovery_code("ABCD-EFGH-JKMN-PQRS-TVWX-YZ0é"),
            None
        );
        assert_eq!(normalise_recovery_code(""), None);
    }

    #[test]
    fn object_id_is_stable_hex_and_key_dependent() {
        let key = VaultKey::generate();
        let id = object_id(&key, "templates/standup.md");
        assert_eq!(id.len(), 64);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );
        assert_eq!(object_id(&key, "templates/standup.md"), id);
        assert_ne!(object_id(&key, "templates/retro.md"), id);
        assert_ne!(object_id(&VaultKey::generate(), "templates/standup.md"), id);

        let fixed = VaultKey([7u8; 32]);
        assert_eq!(
            object_id(&fixed, "templates/standup.md"),
            object_id(
                &VaultKey::from_b64(&fixed.to_b64()).unwrap(),
                "templates/standup.md"
            )
        );
    }

    #[test]
    fn object_id_known_answer_uses_derived_id_key() {
        // Computed independently: HMAC-SHA256(HMAC-SHA256([7; 32], "pennel/object-id/v1"),
        // "templates/standup.md"). A change here re-keys every object on the server.
        let key = VaultKey([7u8; 32]);
        assert_eq!(
            object_id(&key, "templates/standup.md"),
            "37fd5c3fe6453428e919a3b447a64a95ec06b4ce27277ea2bb9f0cb62a4374fe"
        );
        let raw_key_id = "04f1c96e4cbc21d77767c05414ae912920afefff218efd3555d169627906945b";
        assert_ne!(object_id(&key, "templates/standup.md"), raw_key_id);
    }

    #[test]
    fn seal_uses_derived_encryption_key() {
        let key = VaultKey::generate();
        let id = object_id(&key, &payload().key);
        let ad = object_ad(&id, 1);
        let blob = seal(&key, &id, 1, &payload());
        assert!(decrypt(&key.0, &ad, &blob).is_none());
        let plaintext = serde_json::to_vec(&payload()).unwrap();
        let raw_key_blob = encrypt(&key.0, &ad, &plaintext);
        assert!(open(&key, &id, 1, &raw_key_blob).is_err());
    }

    #[test]
    fn open_rejects_payload_key_that_maps_to_other_id() {
        let key = VaultKey::generate();
        let claimed_id = object_id(&key, "templates/b.md");
        let swapped = Payload {
            key: "templates/a.md".into(),
            ..payload()
        };
        let blob = seal(&key, &claimed_id, 1, &swapped);
        assert_eq!(
            open(&key, &claimed_id, 1, &blob).unwrap_err(),
            object_unreadable()
        );
    }

    #[test]
    fn object_id_nfc_equals_nfd() {
        let key = VaultKey::generate();
        let nfc = "meeting/abc/Caf\u{e9}.md";
        let nfd = "meeting/abc/Cafe\u{301}.md";
        assert_ne!(nfc, nfd);
        assert_eq!(object_id(&key, nfc), object_id(&key, nfd));
    }

    #[test]
    fn seal_open_roundtrip() {
        let key = VaultKey::generate();
        let id = object_id(&key, &payload().key);
        let blob = seal(&key, &id, 3, &payload());
        assert_eq!(blob[0], 0x01);
        let plaintext = serde_json::to_vec(&payload()).unwrap();
        assert!(!blob.windows(6).any(|w| w == b"Geheim"));
        assert!(!blob.windows(4).any(|w| w == b"Caf\xc3\xa9"));
        assert_eq!(blob.len(), 1 + 24 + plaintext.len() + 16);
        assert!(open(&key, &id, 3, &blob).unwrap() == payload());

        let empty = Payload {
            folder: None,
            content: Vec::new(),
            ..payload()
        };
        let blob = seal(&key, &id, 0, &empty);
        assert!(open(&key, &id, 0, &blob).unwrap() == empty);

        let again = seal(&key, &id, 3, &payload());
        assert_ne!(seal(&key, &id, 3, &payload())[1..25], again[1..25]);
    }

    #[test]
    fn payload_json_uses_base64_content() {
        let json = serde_json::to_value(payload()).unwrap();
        assert_eq!(json["content"], base64_of(b"# Notities\n\nGeheim."));
        assert_eq!(json["folder"], "2026-10-25 1400 Café overleg");
        assert_eq!(json["modified_ms"], 1_761_400_000_000_i64);
        assert!(!format!("{:?}", payload()).contains("Geheim"));
    }

    #[test]
    fn open_rejects_other_object_id_or_generation() {
        let key = VaultKey::generate();
        let id = object_id(&key, &payload().key);
        let other_id = object_id(&key, "templates/other.md");
        let blob = seal(&key, &id, 3, &payload());
        assert!(open(&key, &other_id, 3, &blob).is_err());
        assert!(open(&key, &id, 2, &blob).is_err());
        assert!(open(&key, &id, 4, &blob).is_err());
        assert!(open(&VaultKey::generate(), &id, 3, &blob).is_err());
    }

    #[test]
    fn open_rejects_tampered_ciphertext() {
        let key = VaultKey::generate();
        let id = object_id(&key, &payload().key);
        let blob = seal(&key, &id, 1, &payload());
        for index in [0, 1, 24, 25, blob.len() / 2, blob.len() - 1] {
            let mut tampered = blob.clone();
            tampered[index] ^= 0x01;
            assert!(open(&key, &id, 1, &tampered).is_err(), "byte {index}");
        }
        for length in [0, 1, 24, 25, 40, blob.len() - 1] {
            assert!(
                open(&key, &id, 1, &blob[..length]).is_err(),
                "length {length}"
            );
        }
        let mut extended = blob.clone();
        extended.push(0);
        assert!(open(&key, &id, 1, &extended).is_err());
    }

    #[test]
    fn open_rejects_authentic_but_invalid_payload() {
        let key = VaultKey::generate();
        let id = object_id(&key, "templates/x.md");
        let blob = encrypt(&enc_key(&key), &object_ad(&id, 1), b"not json");
        assert!(open(&key, &id, 1, &blob).is_err());
    }
}
