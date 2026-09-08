use argon2::Argon2;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use solana_keypair::Keypair;
use solana_signer::Signer;
use std::{fs, path::Path};
use zeroize::Zeroizing;

#[derive(Serialize, Deserialize)]
pub struct Credentials {
    pub rpc_url: String,
    pub secondary_rpc_url: String,
    pub jupiter_api_key: String,
    pub keypair: Vec<u8>,
}
impl Drop for Credentials {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.rpc_url.zeroize();
        self.secondary_rpc_url.zeroize();
        self.jupiter_api_key.zeroize();
        self.keypair.zeroize();
    }
}

#[derive(Serialize, Deserialize)]
struct EncryptedVault {
    version: u8,
    public_key: String,
    salt: [u8; 16],
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}

pub fn public_key(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let vault: EncryptedVault = serde_json::from_slice(&bytes).ok()?;
    Some(vault.public_key)
}

pub fn create(path: &Path, credentials: Credentials, password: &str) -> Result<String, String> {
    if path.exists() {
        return Err(
            "A wallet vault already exists; replacement is disabled to protect its identity".into(),
        );
    }
    if password.len() < 12 || password.len() > 256 {
        return Err("Use a vault passphrase of 12–256 characters".into());
    }
    let keypair = Keypair::try_from(credentials.keypair.as_slice())
        .map_err(|_| "Invalid dedicated Solana keypair")?;
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, key.as_mut())
        .map_err(|_| "Key derivation failed")?;
    let plaintext =
        Zeroizing::new(serde_json::to_vec(&credentials).map_err(|_| "Vault encoding failed")?);
    let cipher =
        ChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| "Vault cipher failed")?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_slice())
        .map_err(|_| "Vault encryption failed")?;
    let public_key = keypair.pubkey().to_string();
    let vault = EncryptedVault {
        version: 1,
        public_key: public_key.clone(),
        salt,
        nonce,
        ciphertext,
    };
    // create_new prevents concurrent wallet replacement, even across processes.
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "Cannot create vault")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| "Cannot secure vault permissions")?;
    }
    file.write_all(&serde_json::to_vec(&vault).map_err(|_| "Vault encoding failed")?)
        .map_err(|_| "Cannot persist vault")?;
    file.sync_all().map_err(|_| "Cannot flush vault")?;
    Ok(public_key)
}

pub fn unlock(path: &Path, password: &str) -> Result<Credentials, String> {
    if password.len() > 256 {
        return Err("Invalid passphrase".into());
    }
    let vault: EncryptedVault =
        serde_json::from_slice(&fs::read(path).map_err(|_| "Vault not configured")?)
            .map_err(|_| "Invalid vault")?;
    if vault.version != 1 {
        return Err("Unsupported vault version".into());
    }
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::default()
        .hash_password_into(password.as_bytes(), &vault.salt, key.as_mut())
        .map_err(|_| "Key derivation failed")?;
    let cipher =
        ChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| "Vault cipher failed")?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(Nonce::from_slice(&vault.nonce), vault.ciphertext.as_slice())
            .map_err(|_| "Incorrect passphrase or corrupted vault")?,
    );
    let credentials: Credentials =
        serde_json::from_slice(&plaintext).map_err(|_| "Invalid vault payload")?;
    let signer =
        Keypair::try_from(credentials.keypair.as_slice()).map_err(|_| "Invalid vault signer")?;
    if signer.pubkey().to_string() != vault.public_key {
        return Err("Vault identity mismatch".into());
    }
    Ok(credentials)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_roundtrip_rejects_wrong_password_and_overwrite() {
        let path =
            std::env::temp_dir().join(format!("kairos-vault-{}.json", rand::random::<u64>()));
        let creds = Credentials {
            rpc_url: "https://example.com".into(),
            secondary_rpc_url: "https://example.org".into(),
            jupiter_api_key: "secret-canary".into(),
            keypair: Keypair::new().to_bytes().to_vec(),
        };
        create(&path, creds, "test-passphrase-123").unwrap();
        assert!(!fs::read_to_string(&path).unwrap().contains("secret-canary"));
        assert!(unlock(&path, "wrong").is_err());
        let decrypted = unlock(&path, "test-passphrase-123").unwrap();
        assert_eq!(decrypted.jupiter_api_key, "secret-canary");
        assert!(create(&path, decrypted, "test-passphrase-123").is_err());
        fs::remove_file(path).unwrap();
    }
}
