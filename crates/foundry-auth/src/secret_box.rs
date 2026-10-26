//! Sealed credentials at rest (ADR-EIP-004, DDR-9).
//!
//! [`SecretBox`] seals outside credentials (Slack bot token, Matrix access
//! token, ...) with AES-256-GCM (`ring`) under the operator-held
//! `FOUNDRY_SECRETS_KEY`, and opens them under that key or a retired one
//! listed in `FOUNDRY_SECRETS_KEY_PREVIOUS`. No I/O: `seal` returns a value,
//! `open` returns a [`SecretString`].
//!
//! Sealed format (version 1): `0x01 | key id (8) | nonce (12) | ciphertext | tag (16)`.
//! The associated data binds a ciphertext to where it lives
//! (`foundry-secret|v1|{table}|{row id}|{column}`), so a value copied into
//! another row, table or column does not open.

use base64::Engine;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::rand::{SecureRandom, SystemRandom};
use secrecy::zeroize::Zeroizing;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// The setting holding the current key: 32 random bytes, base64url.
pub const SECRETS_KEY_SETTING: &str = "FOUNDRY_SECRETS_KEY";
/// The setting holding retired keys (comma-separated) accepted for opening only.
pub const SECRETS_KEY_PREVIOUS_SETTING: &str = "FOUNDRY_SECRETS_KEY_PREVIOUS";

const FORMAT_VERSION: u8 = 1;
const KEY_LEN: usize = 32;
const KEY_ID_LEN: usize = 8;
const KEY_ID_LABEL: &[u8] = b"foundry-secret-key-id|v1|";
/// Where the nonce starts: after the version byte and the key id.
const NONCE_AT: usize = 1 + KEY_ID_LEN;
const HEADER_LEN: usize = NONCE_AT + NONCE_LEN;

/// Why sealing, opening or reading a key failed. No variant carries a key,
/// a key id or a credential, so the text is safe to log.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SecretBoxError {
    /// The named setting does not decode to exactly 32 bytes of base64url.
    #[error("{setting} is not 32 bytes of base64url")]
    MalformedKey { setting: &'static str },
    /// The system random source or the cipher failed while sealing.
    #[error("the credential could not be sealed")]
    SealFailed,
    /// Moved, altered, truncated or sealed under a key this box does not hold.
    #[error("the sealed credential could not be opened")]
    OpenRefused,
}

/// The public identity of a key: the first 8 bytes of SHA-256 over a fixed
/// label and the key. Not secret; health lines name missing key ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyId([u8; KEY_ID_LEN]);

impl KeyId {
    fn derive(key: &[u8]) -> Self {
        let digest = Sha256::new()
            .chain_update(KEY_ID_LABEL)
            .chain_update(key)
            .finalize();
        let mut id = [0u8; KEY_ID_LEN];
        id.copy_from_slice(&digest[..KEY_ID_LEN]);
        Self(id)
    }
}

impl std::fmt::Display for KeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

/// One AES-256-GCM key read from a setting. Holds no printable material.
pub struct SecretsKey {
    id: KeyId,
    cipher: LessSafeKey,
}

impl SecretsKey {
    /// Read a key from its setting value: base64url, padded or not, that
    /// decodes to exactly 32 bytes. The error names `setting`, never the value.
    pub fn parse(encoded: &SecretString, setting: &'static str) -> Result<Self, SecretBoxError> {
        let malformed = SecretBoxError::MalformedKey { setting };
        let engine = base64::engine::GeneralPurpose::new(
            &base64::alphabet::URL_SAFE,
            base64::engine::GeneralPurposeConfig::new()
                .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
        );
        let bytes = Zeroizing::new(
            engine
                .decode(encoded.expose_secret())
                .map_err(|_| malformed.clone())?,
        );
        if bytes.len() != KEY_LEN {
            return Err(malformed);
        }
        let cipher = UnboundKey::new(&AES_256_GCM, &bytes).map_err(|_| malformed)?;
        Ok(Self {
            id: KeyId::derive(&bytes),
            cipher: LessSafeKey::new(cipher),
        })
    }

    pub fn key_id(&self) -> KeyId {
        self.id
    }
}

/// Where a sealed value lives (table, row id, column): the associated data.
pub struct SecretSlot {
    aad: String,
}

impl SecretSlot {
    pub fn new(table: &str, row_id: &str, column: &str) -> Self {
        Self {
            aad: format!("foundry-secret|v1|{table}|{row_id}|{column}"),
        }
    }

    fn aad(&self) -> Aad<&[u8]> {
        Aad::from(self.aad.as_bytes())
    }
}

/// An opaque sealed credential, stored as `bytea`. Deliberately implements
/// none of `Debug`, `Display` or `Serialize`.
pub struct SealedSecret(Vec<u8>);

impl SealedSecret {
    /// Wrap bytes read back from storage. Validity is checked on `open`.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The bytes to store.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The id of the key this value claims to be sealed under (all zeros when
    /// the value is too short to carry one).
    pub fn key_id(&self) -> KeyId {
        let mut id = [0u8; KEY_ID_LEN];
        if let Some(stored) = self.0.get(1..=KEY_ID_LEN) {
            id.copy_from_slice(stored);
        }
        KeyId(id)
    }
}

/// The retired keys of a comma-separated `FOUNDRY_SECRETS_KEY_PREVIOUS`
/// value, each entry trimmed and blank entries ignored.
fn previous_keys(list: &SecretString) -> Result<Vec<SecretsKey>, SecretBoxError> {
    list.expose_secret()
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            SecretsKey::parse(
                &SecretString::from(entry.to_string()),
                SECRETS_KEY_PREVIOUS_SETTING,
            )
        })
        .collect()
}

/// Seals under the current key; opens under the current or a previous key.
pub struct SecretBox {
    current: SecretsKey,
    previous: Vec<SecretsKey>,
    random: SystemRandom,
}

impl SecretBox {
    pub fn new(current: SecretsKey, previous: Vec<SecretsKey>) -> Self {
        Self {
            current,
            previous,
            random: SystemRandom::new(),
        }
    }

    /// Build from the `FOUNDRY_SECRETS_KEY` value and the optional
    /// comma-separated `FOUNDRY_SECRETS_KEY_PREVIOUS` value (entries are
    /// trimmed, blank entries ignored). A malformed key is refused naming its
    /// setting only.
    pub fn from_settings(
        current: &SecretString,
        previous: Option<&SecretString>,
    ) -> Result<Self, SecretBoxError> {
        let current = SecretsKey::parse(current, SECRETS_KEY_SETTING)?;
        let previous = previous.map(previous_keys).transpose()?.unwrap_or_default();
        Ok(Self::new(current, previous))
    }

    pub fn current_key_id(&self) -> KeyId {
        self.current.id
    }

    /// The ids of every key this box opens with, current first.
    pub fn key_ids(&self) -> Vec<KeyId> {
        self.keys().map(SecretsKey::key_id).collect()
    }

    pub fn seal(
        &self,
        plaintext: &SecretString,
        slot: &SecretSlot,
    ) -> Result<SealedSecret, SecretBoxError> {
        let mut nonce = [0u8; NONCE_LEN];
        self.random
            .fill(&mut nonce)
            .map_err(|_| SecretBoxError::SealFailed)?;
        let mut body = plaintext.expose_secret().as_bytes().to_vec();
        self.current
            .cipher
            .seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), slot.aad(), &mut body)
            .map_err(|_| SecretBoxError::SealFailed)?;
        let mut sealed = Vec::with_capacity(HEADER_LEN + body.len());
        sealed.push(FORMAT_VERSION);
        sealed.extend_from_slice(&self.current.id.0);
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(&body);
        Ok(SealedSecret(sealed))
    }

    pub fn open(
        &self,
        sealed: &SealedSecret,
        slot: &SecretSlot,
    ) -> Result<SecretString, SecretBoxError> {
        let (header, body) = sealed
            .as_bytes()
            .split_at_checked(HEADER_LEN)
            .ok_or(SecretBoxError::OpenRefused)?;
        if header[0] != FORMAT_VERSION {
            return Err(SecretBoxError::OpenRefused);
        }
        let key_id = sealed.key_id();
        let key = self
            .keys()
            .find(|key| key.id == key_id)
            .ok_or(SecretBoxError::OpenRefused)?;
        let nonce = Nonce::try_assume_unique_for_key(&header[NONCE_AT..])
            .map_err(|_| SecretBoxError::OpenRefused)?;
        let mut body = Zeroizing::new(body.to_vec());
        let plaintext = key
            .cipher
            .open_in_place(nonce, slot.aad(), &mut body)
            .map_err(|_| SecretBoxError::OpenRefused)?;
        let text = std::str::from_utf8(plaintext).map_err(|_| SecretBoxError::OpenRefused)?;
        Ok(SecretString::from(text.to_string()))
    }

    fn keys(&self) -> impl Iterator<Item = &SecretsKey> {
        std::iter::once(&self.current).chain(&self.previous)
    }
}
