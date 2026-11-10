//! Fichier de clé : la DEK (32 octets aléatoires) enveloppée par une KEK
//! dérivée de la phrase secrète avec Argon2id, chiffrée en XChaCha20-Poly1305.
//! L'en-tête (version, paramètres, sel) est authentifié comme donnée associée :
//! toute altération, y compris un affaiblissement des paramètres, est rejetée.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{CoreError, Result};

pub const DEK_LEN: usize = 32;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const FORMAT_VERSION: u32 = 1;
/// Longueur minimale en caractères. La vraie défense est Argon2id ; ce seuil
/// évite seulement les phrases manifestement triviales.
pub const MIN_PASSPHRASE_CHARS: usize = 8;

pub type Dek = Zeroizing<[u8; DEK_LEN]>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Mémoire en Kio.
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    /// Recommandation OWASP 2025 pour Argon2id, réglée pour ~0,5 s sur un PC courant.
    pub const DEFAULT: Self = Self {
        m_cost: 64 * 1024,
        t_cost: 3,
        p_cost: 1,
    };
    /// Plancher accepté à la lecture : protège contre un fichier forgé
    /// avec des paramètres dérisoires.
    const FLOOR: Self = Self {
        m_cost: 19 * 1024,
        t_cost: 2,
        p_cost: 1,
    };

    #[cfg(test)]
    pub const TEST: Self = Self::FLOOR;

    fn is_acceptable(self) -> bool {
        self.m_cost >= Self::FLOOR.m_cost
            && self.t_cost >= Self::FLOOR.t_cost
            && (1..=16).contains(&self.p_cost)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyFile {
    pub version: u32,
    pub kdf: KdfParams,
    #[serde(with = "hex::serde")]
    pub salt: Vec<u8>,
    #[serde(with = "hex::serde")]
    pub nonce: Vec<u8>,
    #[serde(with = "hex::serde")]
    pub wrapped_dek: Vec<u8>,
}

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|_| CoreError::Crypto)?;
    Ok(b)
}

fn derive_kek(passphrase: &str, salt: &[u8], p: KdfParams) -> Result<Zeroizing<[u8; 32]>> {
    let params =
        Params::new(p.m_cost, p.t_cost, p.p_cost, Some(32)).map_err(|_| CoreError::Crypto)?;
    let mut kek = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, kek.as_mut())
        .map_err(|_| CoreError::Crypto)?;
    Ok(kek)
}

fn aad(version: u32, kdf: KdfParams, salt: &[u8]) -> Vec<u8> {
    let mut v = b"pluralhome/keyfile".to_vec();
    for n in [version, kdf.m_cost, kdf.t_cost, kdf.p_cost] {
        v.extend_from_slice(&n.to_le_bytes());
    }
    v.extend_from_slice(salt);
    v
}

impl KeyFile {
    /// Génère une nouvelle DEK et l'enveloppe avec la phrase secrète.
    pub fn create(passphrase: &str, kdf: KdfParams) -> Result<(Self, Dek)> {
        let dek: Dek = Zeroizing::new(random::<DEK_LEN>()?);
        let kf = Self::wrap(passphrase, kdf, &dek)?;
        Ok((kf, dek))
    }

    /// Enveloppe une DEK existante (création, changement de phrase secrète).
    pub fn wrap(passphrase: &str, kdf: KdfParams, dek: &Dek) -> Result<Self> {
        if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
            return Err(CoreError::WeakPassphrase);
        }
        let salt = random::<SALT_LEN>()?;
        let nonce = random::<NONCE_LEN>()?;
        let kek = derive_kek(passphrase, &salt, kdf)?;
        let cipher = XChaCha20Poly1305::new(&Key::from(*kek));
        let wrapped = cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: dek.as_slice(),
                    aad: &aad(FORMAT_VERSION, kdf, &salt),
                },
            )
            .map_err(|_| CoreError::Crypto)?;
        Ok(Self {
            version: FORMAT_VERSION,
            kdf,
            salt: salt.to_vec(),
            nonce: nonce.to_vec(),
            wrapped_dek: wrapped,
        })
    }

    pub fn unwrap_dek(&self, passphrase: &str) -> Result<Dek> {
        if self.version != FORMAT_VERSION
            || !self.kdf.is_acceptable()
            || self.salt.len() != SALT_LEN
        {
            return Err(CoreError::CorruptKeyFile);
        }
        let nonce: [u8; NONCE_LEN] = self
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| CoreError::CorruptKeyFile)?;
        let kek = derive_kek(passphrase, &self.salt, self.kdf)?;
        let cipher = XChaCha20Poly1305::new(&Key::from(*kek));
        let plain = Zeroizing::new(
            cipher
                .decrypt(
                    &XNonce::from(nonce),
                    Payload {
                        msg: &self.wrapped_dek,
                        aad: &aad(self.version, self.kdf, &self.salt),
                    },
                )
                // Mauvaise phrase et fichier altéré sont indiscernables ici, par construction.
                .map_err(|_| CoreError::BadPassphrase)?,
        );
        let arr: [u8; DEK_LEN] = plain
            .as_slice()
            .try_into()
            .map_err(|_| CoreError::CorruptKeyFile)?;
        Ok(Zeroizing::new(arr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_wrong_passphrase() {
        let (kf, dek) = KeyFile::create("correct horse battery", KdfParams::TEST).unwrap();
        assert_eq!(*kf.unwrap_dek("correct horse battery").unwrap(), *dek);
        assert!(matches!(
            kf.unwrap_dek("wrong passphrase"),
            Err(CoreError::BadPassphrase)
        ));
    }

    #[test]
    fn rejects_weakened_params() {
        let (mut kf, _) = KeyFile::create("correct horse battery", KdfParams::TEST).unwrap();
        kf.kdf.t_cost = 1;
        assert!(matches!(
            kf.unwrap_dek("correct horse battery"),
            Err(CoreError::CorruptKeyFile)
        ));
    }

    #[test]
    fn rejects_tampered_header() {
        let (mut kf, _) = KeyFile::create("correct horse battery", KdfParams::TEST).unwrap();
        kf.kdf.m_cost += 1024;
        assert!(kf.unwrap_dek("correct horse battery").is_err());
    }

    #[test]
    fn rejects_short_passphrase() {
        assert!(matches!(
            KeyFile::create("court", KdfParams::TEST),
            Err(CoreError::WeakPassphrase)
        ));
    }
}
