//! Clés de la synchronisation entre appareils. Le serveur ne voit que des blobs
//! chiffrés et des identifiants opaques :
//! - clé de synchro K (32 octets, aléatoire), propre au système, gardée dans le coffre ;
//! - identifiant d'un élément = `HMAC(K_mac, table | clé primaire)` ;
//! - contenu = XChaCha20-Poly1305(K_enc), lié à son identifiant (données associées) ;
//! - clé de secours R (128 bits, montrée une seule fois) : enveloppe K sur le serveur ;
//! - appairage : le nouvel appareil publie une clé publique X25519, un appareil déjà
//!   synchronisé y scelle K ; une empreinte affichée des deux côtés déjoue une substitution.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::{CoreError, Result};

const NONCE: usize = 24;
/// Base32 de Crockford : sans I, L, O, U.
const B32: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|_| CoreError::Crypto)?;
    Ok(b)
}

fn derive(ikm: &[u8], info: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut out = Zeroizing::new([0u8; 32]);
    // 32 octets : toujours dans les limites de HKDF-SHA256.
    let _ = Hkdf::<Sha256>::new(None, ikm).expand(info, &mut *out);
    out
}

fn seal_with(key: &[u8; 32], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>> {
    let nonce = random::<NONCE>()?;
    let ct = XChaCha20Poly1305::new(&Key::from(*key))
        .encrypt(&XNonce::from(nonce), Payload { msg: plain, aad })
        .map_err(|_| CoreError::Crypto)?;
    let mut out = Vec::with_capacity(NONCE + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

fn open_with(key: &[u8; 32], aad: &[u8], sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if sealed.len() < NONCE + 16 {
        return Err(CoreError::Crypto);
    }
    let (nonce, ct) = sealed.split_at(NONCE);
    let nonce: [u8; NONCE] = nonce.try_into().map_err(|_| CoreError::Crypto)?;
    XChaCha20Poly1305::new(&Key::from(*key))
        .decrypt(&XNonce::from(nonce), Payload { msg: ct, aad })
        .map(Zeroizing::new)
        .map_err(|_| CoreError::Crypto)
}

/// Clé de synchro du système.
pub struct SyncKey {
    raw: Zeroizing<[u8; 32]>,
    enc: Zeroizing<[u8; 32]>,
    mac: Zeroizing<[u8; 32]>,
}

impl SyncKey {
    pub fn generate() -> Result<Self> {
        Ok(Self::from_bytes(random::<32>()?))
    }

    #[must_use]
    pub fn from_bytes(raw: [u8; 32]) -> Self {
        let raw = Zeroizing::new(raw);
        Self {
            enc: derive(&*raw, b"pluralhome sync enc v1"),
            mac: derive(&*raw, b"pluralhome sync mac v1"),
            raw,
        }
    }

    pub fn from_hex(s: &str) -> Result<Self> {
        let mut raw = [0u8; 32];
        hex::decode_to_slice(s, &mut raw).map_err(|_| CoreError::Crypto)?;
        Ok(Self::from_bytes(raw))
    }

    #[must_use]
    pub fn to_hex(&self) -> Zeroizing<String> {
        Zeroizing::new(hex::encode(*self.raw))
    }

    /// Identifiant opaque d'un élément (table + clé primaire).
    #[must_use]
    pub fn item_id(&self, table: &str, pk: &str) -> String {
        let mut m = <Hmac<Sha256> as hmac::KeyInit>::new_from_slice(&*self.mac)
            .unwrap_or_else(|_| unreachable!("HMAC accepte toute longueur de clé"));
        m.update(table.as_bytes());
        m.update(&[0]);
        m.update(pk.as_bytes());
        hex::encode(m.finalize().into_bytes())
    }

    pub fn seal(&self, id: &str, plain: &[u8]) -> Result<Vec<u8>> {
        seal_with(&self.enc, id.as_bytes(), plain)
    }

    pub fn open(&self, id: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        open_with(&self.enc, id.as_bytes(), sealed)
    }

    /// Enveloppe la clé pour le serveur, sous la clé de secours.
    pub fn wrap_for_recovery(&self, recovery: &RecoveryKey, keyset_id: &str) -> Result<Vec<u8>> {
        seal_with(&recovery.kek(), keyset_id.as_bytes(), &*self.raw)
    }

    pub fn unwrap_recovery(
        wrapped: &[u8],
        recovery: &RecoveryKey,
        keyset_id: &str,
    ) -> Result<Self> {
        let plain = open_with(&recovery.kek(), keyset_id.as_bytes(), wrapped)
            .map_err(|_| CoreError::BadPassphrase)?;
        let raw: [u8; 32] = plain.as_slice().try_into().map_err(|_| CoreError::Crypto)?;
        Ok(Self::from_bytes(raw))
    }

    /// Scelle la clé pour un appareil en attente d'appairage. Renvoie
    /// `clé publique éphémère (32) | chiffré`.
    pub fn seal_for_device(&self, device_pub: &[u8; 32], keyset_id: &str) -> Result<Vec<u8>> {
        let eph = StaticSecret::from(random::<32>()?);
        let eph_pub = PublicKey::from(&eph);
        let shared = eph.diffie_hellman(&PublicKey::from(*device_pub));
        let key = pair_key(shared.as_bytes(), eph_pub.as_bytes(), device_pub);
        let mut out = eph_pub.as_bytes().to_vec();
        out.extend(seal_with(&key, keyset_id.as_bytes(), &*self.raw)?);
        Ok(out)
    }
}

fn pair_key(shared: &[u8; 32], eph_pub: &[u8; 32], device_pub: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut ikm = Zeroizing::new(Vec::with_capacity(96));
    ikm.extend_from_slice(shared);
    ikm.extend_from_slice(eph_pub);
    ikm.extend_from_slice(device_pub);
    derive(&ikm, b"pluralhome pairing v1")
}

/// Clé de secours : 128 bits, affichée en 26 caractères (groupes de 4 à 5).
pub struct RecoveryKey(Zeroizing<[u8; 16]>);

impl RecoveryKey {
    pub fn generate() -> Result<Self> {
        Ok(Self(Zeroizing::new(random::<16>()?)))
    }

    fn kek(&self) -> Zeroizing<[u8; 32]> {
        derive(&*self.0, b"pluralhome recovery v1")
    }

    /// Forme lisible (base32 de Crockford), par groupes de 4.
    #[must_use]
    pub fn display(&self) -> Zeroizing<String> {
        let mut bits: u32 = 0;
        let mut n = 0;
        let mut chars = Zeroizing::new(String::new());
        for &b in self.0.iter() {
            bits = (bits << 8) | u32::from(b);
            n += 8;
            while n >= 5 {
                n -= 5;
                chars.push(char::from(B32[((bits >> n) & 31) as usize]));
            }
        }
        if n > 0 {
            chars.push(char::from(B32[((bits << (5 - n)) & 31) as usize]));
        }
        let mut out = Zeroizing::new(String::new());
        for (i, c) in chars.chars().enumerate() {
            if i > 0 && i % 4 == 0 {
                out.push(' ');
            }
            out.push(c);
        }
        out
    }

    /// Relit une clé saisie : casse, espaces et tirets indifférents ; 0/O, 1/I/L confondus.
    pub fn parse(input: &str) -> Result<Self> {
        let mut bits: u32 = 0;
        let mut n = 0;
        let mut out = Zeroizing::new(Vec::with_capacity(17));
        let mut count = 0;
        for c in input.chars() {
            let c = match c.to_ascii_uppercase() {
                ' ' | '-' | '\t' | '\n' | '\r' => continue,
                'O' => '0',
                'I' | 'L' => '1',
                c => c,
            };
            let v = B32
                .iter()
                .position(|&x| char::from(x) == c)
                .ok_or(CoreError::Invalid("recovery_key"))?;
            count += 1;
            bits = (bits << 5) | u32::try_from(v).unwrap_or(0);
            n += 5;
            if n >= 8 {
                n -= 8;
                out.push(((bits >> n) & 0xff) as u8);
            }
        }
        if count != 26 || out.len() != 16 {
            return Err(CoreError::Invalid("recovery_key"));
        }
        let mut raw = [0u8; 16];
        raw.copy_from_slice(&out);
        Ok(Self(Zeroizing::new(raw)))
    }
}

/// Côté nouvel appareil : paire de clés le temps de l'appairage.
pub struct PairingSecret(Zeroizing<[u8; 32]>);

impl PairingSecret {
    pub fn generate() -> Result<Self> {
        Ok(Self(Zeroizing::new(random::<32>()?)))
    }

    pub fn from_hex(s: &str) -> Result<Self> {
        let mut raw = [0u8; 32];
        hex::decode_to_slice(s, &mut raw).map_err(|_| CoreError::Crypto)?;
        Ok(Self(Zeroizing::new(raw)))
    }

    #[must_use]
    pub fn to_hex(&self) -> Zeroizing<String> {
        Zeroizing::new(hex::encode(*self.0))
    }

    #[must_use]
    pub fn public(&self) -> [u8; 32] {
        *PublicKey::from(&StaticSecret::from(*self.0)).as_bytes()
    }

    pub fn open(&self, sealed: &[u8], keyset_id: &str) -> Result<SyncKey> {
        if sealed.len() < 32 {
            return Err(CoreError::Crypto);
        }
        let (eph_pub, rest) = sealed.split_at(32);
        let eph_pub: [u8; 32] = eph_pub.try_into().map_err(|_| CoreError::Crypto)?;
        let me = self.public();
        let shared = StaticSecret::from(*self.0).diffie_hellman(&PublicKey::from(eph_pub));
        let key = pair_key(shared.as_bytes(), &eph_pub, &me);
        let plain = open_with(&key, keyset_id.as_bytes(), rest)?;
        let raw: [u8; 32] = plain.as_slice().try_into().map_err(|_| CoreError::Crypto)?;
        Ok(SyncKey::from_bytes(raw))
    }
}

/// Empreinte d'une clé publique d'appairage, à comparer à l'œil : 6 chiffres.
#[must_use]
pub fn fingerprint(public: &[u8; 32]) -> String {
    let h = Sha256::digest(public);
    let n = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

/// Identité d'amitié du système : une paire X25519 propre, distincte de la clé
/// de synchro. Le serveur ne voit que la clé publique ; tout ce qui passe entre
/// deux systèmes amis est chiffré avec une clé tirée de leur échange Diffie-Hellman.
/// Pas de confidentialité persistante : qui vole cette clé lit l'historique.
pub struct FriendIdentity(Zeroizing<[u8; 32]>);

impl FriendIdentity {
    pub fn generate() -> Result<Self> {
        Ok(Self(Zeroizing::new(random::<32>()?)))
    }

    pub fn from_hex(s: &str) -> Result<Self> {
        let mut raw = [0u8; 32];
        hex::decode_to_slice(s, &mut raw).map_err(|_| CoreError::Crypto)?;
        Ok(Self(Zeroizing::new(raw)))
    }

    #[must_use]
    pub fn to_hex(&self) -> Zeroizing<String> {
        Zeroizing::new(hex::encode(*self.0))
    }

    #[must_use]
    pub fn public(&self) -> [u8; 32] {
        *PublicKey::from(&StaticSecret::from(*self.0)).as_bytes()
    }

    fn key_with(&self, their: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>> {
        let me = self.public();
        if *their == me {
            return Err(CoreError::Crypto);
        }
        let shared = StaticSecret::from(*self.0).diffie_hellman(&PublicKey::from(*their));
        if !shared.was_contributory() {
            return Err(CoreError::Crypto);
        }
        let (lo, hi) = if me < *their {
            (me, *their)
        } else {
            (*their, me)
        };
        let mut ikm = Zeroizing::new([0u8; 96]);
        ikm[..32].copy_from_slice(shared.as_bytes());
        ikm[32..64].copy_from_slice(&lo);
        ikm[64..].copy_from_slice(&hi);
        Ok(derive(&*ikm, b"pluralhome friend v1"))
    }

    /// Chiffre pour un système ami ; `purpose` sépare les usages (fiche, courrier…).
    pub fn seal_for(&self, their: &[u8; 32], purpose: &str, plain: &[u8]) -> Result<Vec<u8>> {
        let key = self.key_with(their)?;
        seal_with(&key, &friend_aad(purpose, &self.public()), plain)
    }

    /// Déchiffre ce qu'un système ami a scellé pour nous.
    pub fn open_from(
        &self,
        their: &[u8; 32],
        purpose: &str,
        sealed: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>> {
        let key = self.key_with(their)?;
        open_with(&key, &friend_aad(purpose, their), sealed)
    }
}

fn friend_aad(purpose: &str, sender: &[u8; 32]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(purpose.len() + 33);
    aad.extend_from_slice(purpose.as_bytes());
    aad.push(0);
    aad.extend_from_slice(sender);
    aad
}

/// Code de vérification d'une amitié : identique des deux côtés, à comparer
/// de vive voix pour s'assurer que le serveur n'a pas glissé une autre clé.
#[must_use]
pub fn friend_code(mine: &[u8; 32], theirs: &[u8; 32]) -> String {
    let (lo, hi) = if mine < theirs {
        (mine, theirs)
    } else {
        (theirs, mine)
    };
    let mut hasher = Sha256::new();
    hasher.update(b"pluralhome friend code v1");
    hasher.update(lo);
    hasher.update(hi);
    let digest = hasher.finalize();
    let mut head = [0u8; 8];
    head.copy_from_slice(&digest[..8]);
    let digits = format!("{:012}", u64::from_be_bytes(head) % 1_000_000_000_000);
    format!("{} {} {}", &digits[..4], &digits[4..8], &digits[8..])
}

/// Secret d'une invitation : il voyage dans le lien ou le QR code, jamais au serveur.
pub struct InviteSecret(Zeroizing<[u8; 16]>);

impl InviteSecret {
    pub fn generate() -> Result<Self> {
        Ok(Self(Zeroizing::new(random::<16>()?)))
    }

    pub fn from_hex(s: &str) -> Result<Self> {
        let mut raw = [0u8; 16];
        hex::decode_to_slice(s, &mut raw).map_err(|_| CoreError::Crypto)?;
        Ok(Self(Zeroizing::new(raw)))
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(*self.0)
    }

    pub fn seal(&self, plain: &[u8]) -> Result<Vec<u8>> {
        seal_with(&derive(&*self.0, b"pluralhome invite v1"), b"invite", plain)
    }

    pub fn open(&self, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        open_with(
            &derive(&*self.0, b"pluralhome invite v1"),
            b"invite",
            sealed,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_roundtrip_and_binding() {
        let k = SyncKey::generate().unwrap();
        let id = k.item_id("member", "[\"AB\"]");
        assert_eq!(id, k.item_id("member", "[\"AB\"]"));
        assert_ne!(id, k.item_id("member", "[\"AC\"]"));
        let s = k.seal(&id, b"secret").unwrap();
        assert_eq!(&*k.open(&id, &s).unwrap(), b"secret");
        assert!(k.open("autre", &s).is_err());
        let other = SyncKey::generate().unwrap();
        assert!(other.open(&id, &s).is_err());
        let again = SyncKey::from_hex(&k.to_hex()).unwrap();
        assert_eq!(again.item_id("t", "p"), k.item_id("t", "p"));
    }

    #[test]
    fn recovery_display_parse() {
        for _ in 0..50 {
            let r = RecoveryKey::generate().unwrap();
            let shown = r.display();
            let back = RecoveryKey::parse(&shown.to_lowercase().replace(' ', "-")).unwrap();
            assert_eq!(*back.0, *r.0);
        }
        assert!(RecoveryKey::parse("ABCD").is_err());
        let k = SyncKey::generate().unwrap();
        let r = RecoveryKey::generate().unwrap();
        let w = k.wrap_for_recovery(&r, "ks1").unwrap();
        let back = SyncKey::unwrap_recovery(&w, &r, "ks1").unwrap();
        assert_eq!(*back.raw, *k.raw);
        assert!(SyncKey::unwrap_recovery(&w, &RecoveryKey::generate().unwrap(), "ks1").is_err());
        assert!(SyncKey::unwrap_recovery(&w, &r, "ks2").is_err());
    }

    #[test]
    fn pairing_roundtrip() {
        let k = SyncKey::generate().unwrap();
        let dev = PairingSecret::generate().unwrap();
        let sealed = k.seal_for_device(&dev.public(), "ks").unwrap();
        let got = dev.open(&sealed, "ks").unwrap();
        assert_eq!(*got.raw, *k.raw);
        let intruder = PairingSecret::generate().unwrap();
        assert!(intruder.open(&sealed, "ks").is_err());
        assert_eq!(fingerprint(&dev.public()).len(), 7);
    }

    #[test]
    fn friend_roundtrip() {
        let a = FriendIdentity::generate().unwrap();
        let b = FriendIdentity::generate().unwrap();
        let s = a.seal_for(&b.public(), "card", b"hello").unwrap();
        assert_eq!(&*b.open_from(&a.public(), "card", &s).unwrap(), b"hello");
        assert!(b.open_from(&a.public(), "mail", &s).is_err());
        let c = FriendIdentity::generate().unwrap();
        assert!(c.open_from(&a.public(), "card", &s).is_err());
        // Ce qu'on a scellé soi-même ne se relit pas comme venant de l'autre.
        assert!(a.open_from(&b.public(), "card", &s).is_err());
        assert!(a.seal_for(&a.public(), "card", b"x").is_err());
        assert!(a.seal_for(&[0u8; 32], "card", b"x").is_err());
        let again = FriendIdentity::from_hex(&a.to_hex()).unwrap();
        assert_eq!(again.public(), a.public());
        let code = friend_code(&a.public(), &b.public());
        assert_eq!(code, friend_code(&b.public(), &a.public()));
        assert_eq!(code.len(), 14);

        let inv = InviteSecret::generate().unwrap();
        let sealed = inv.seal(b"card").unwrap();
        let back = InviteSecret::from_hex(&inv.to_hex()).unwrap();
        assert_eq!(&*back.open(&sealed).unwrap(), b"card");
        assert!(InviteSecret::generate().unwrap().open(&sealed).is_err());
    }
}
