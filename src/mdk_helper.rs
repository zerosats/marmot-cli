use anyhow::{Context, Result};
use mdk_core::prelude::*;
use mdk_sqlite_storage::MdkSqliteStorage;
use nostr_sdk::prelude::*;
use nostr_sdk::ToBech32;
use std::path::Path;

use crate::config::Config;

pub struct MdkContext {
    pub mdk: MDK<MdkSqliteStorage>,
    pub keys: Keys,
    pub relays: Vec<RelayUrl>,
}

impl MdkContext {
    pub fn load(config: &Config) -> Result<Self> {
        let keys = load_keys(config)?;
        let storage = MdkSqliteStorage::new_unencrypted(&config.db_path)
            .context("Failed to initialize MDK SQLite storage")?;
        let mdk = MDK::new(storage);
        let relays = config
            .relays
            .iter()
            .filter_map(|r| RelayUrl::parse(r).ok())
            .collect();

        Ok(Self { mdk, keys, relays })
    }

    pub fn pubkey(&self) -> PublicKey {
        self.keys.public_key()
    }

    pub fn npub(&self) -> String {
        self.keys.public_key().to_bech32().unwrap_or_default()
    }
}

pub fn load_keys(config: &Config) -> Result<Keys> {
    let key_file = config
        .key_file
        .as_ref()
        .context("No key file specified. Run 'mdk init' first or use --key-file")?;

    let content = std::fs::read_to_string(key_file)
        .with_context(|| format!("Failed to read key file: {:?}", key_file))?;

    let secret_key = parse_secret_key(content.trim())?;
    Ok(Keys::new(secret_key))
}

pub fn parse_secret_key(input: &str) -> Result<SecretKey> {
    if input.starts_with("nsec") {
        SecretKey::from_bech32(input).context("Invalid nsec format")
    } else {
        SecretKey::from_hex(input).context("Invalid hex secret key")
    }
}

pub fn generate_keys() -> Keys {
    Keys::generate()
}

pub fn save_keys(keys: &Keys, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {:?}", parent))?;
    }

    let secret_hex = keys.secret_key().to_secret_hex();
    std::fs::write(path, &secret_hex)
        .with_context(|| format!("Failed to write key file: {:?}", path))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }

    Ok(())
}

pub fn db_exists(config: &Config) -> bool {
    config.db_path.exists()
}
