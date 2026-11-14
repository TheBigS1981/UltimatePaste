//! At-rest encryption. A random 64-byte master secret (32 B AES key + 32 B HMAC key)
//! is stored in `vault.key`, wrapped with DPAPI so only the current Windows user can unwrap it.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Context, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::path::Path;

use crate::platform;

const NONCE_LEN: usize = 12;

pub struct Vault {
    cipher: Aes256Gcm,
    mac_key: [u8; 32],
}

impl Vault {
    pub fn open(dir: &Path) -> Result<Self> {
        let path = dir.join("vault.key");
        let secret = if path.exists() {
            let wrapped = std::fs::read(&path).context("read vault.key")?;
            platform::unprotect(&wrapped).context("unwrap vault.key (DPAPI)")?
        } else {
            let mut s = rand::random::<[u8; 32]>().to_vec();
            s.extend_from_slice(&rand::random::<[u8; 32]>());
            let wrapped = platform::protect(&s)?;
            std::fs::write(&path, wrapped).context("write vault.key")?;
            s
        };
        if secret.len() != 64 {
            return Err(anyhow!("vault.key has unexpected length"));
        }
        Ok(Self::from_secret(&secret))
    }

    pub fn from_secret(secret: &[u8]) -> Self {
        let cipher = Aes256Gcm::new_from_slice(&secret[..32]).expect("32 byte key");
        let mut mac_key = [0u8; 32];
        mac_key.copy_from_slice(&secret[32..64]);
        Self { cipher, mac_key }
    }

    pub fn encrypt(&self, plain: &[u8]) -> Vec<u8> {
        let nonce: [u8; NONCE_LEN] = rand::random();
        let ct = self
            .cipher
            .encrypt(&Nonce::from(nonce), plain)
            .expect("AES-GCM encryption cannot fail for in-memory buffers");
        let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        out
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < NONCE_LEN {
            return Err(anyhow!("ciphertext too short"));
        }
        let (nonce, ct) = data.split_at(NONCE_LEN);
        let nonce: [u8; NONCE_LEN] = nonce.try_into().expect("checked length");
        self.cipher
            .decrypt(&Nonce::from(nonce), ct)
            .map_err(|_| anyhow!("decryption failed"))
    }

    pub fn encrypt_str(&self, s: &str) -> Vec<u8> {
        self.encrypt(s.as_bytes())
    }

    pub fn decrypt_string(&self, data: &[u8]) -> Result<String> {
        Ok(String::from_utf8(self.decrypt(data)?)?)
    }

    /// Keyed hash, so stored hashes reveal nothing about the content.
    pub fn keyed_hash(&self, parts: &[&[u8]]) -> [u8; 32] {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&self.mac_key).expect("any key size");
        for p in parts {
            mac.update(&(p.len() as u64).to_le_bytes());
            mac.update(p);
        }
        mac.finalize().into_bytes().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let v = Vault::from_secret(&[7u8; 64]);
        let ct = v.encrypt(b"secret text");
        assert_ne!(&ct[NONCE_LEN..], b"secret text");
        assert_eq!(v.decrypt(&ct).unwrap(), b"secret text");
    }

    #[test]
    fn tamper_detected() {
        let v = Vault::from_secret(&[7u8; 64]);
        let mut ct = v.encrypt(b"abc");
        let last = ct.len() - 1;
        ct[last] ^= 1;
        assert!(v.decrypt(&ct).is_err());
    }

    #[test]
    fn hash_is_keyed_and_unambiguous() {
        let a = Vault::from_secret(&[1u8; 64]);
        let b = Vault::from_secret(&[2u8; 64]);
        assert_ne!(a.keyed_hash(&[b"x"]), b.keyed_hash(&[b"x"]));
        assert_ne!(a.keyed_hash(&[b"ab", b"c"]), a.keyed_hash(&[b"a", b"bc"]));
    }

    #[test]
    fn open_creates_and_reopens() {
        let dir = std::env::temp_dir().join(format!("up-vault-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ct = Vault::open(&dir).unwrap().encrypt(b"persist");
        assert_eq!(Vault::open(&dir).unwrap().decrypt(&ct).unwrap(), b"persist");
        std::fs::remove_dir_all(&dir).ok();
    }
}
