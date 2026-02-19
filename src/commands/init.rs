use anyhow::{Context, Result};
use mdk_sqlite_storage::MdkSqliteStorage;
use nostr_sdk::ToBech32;
use serde::Serialize;
use std::path::PathBuf;

use crate::config::Config;
use crate::mdk_helper::{generate_keys, parse_secret_key, save_keys};
use crate::output::print_json;

#[derive(Serialize)]
struct InitOutput {
    pubkey: String,
    npub: String,
    key_file: String,
    db_path: String,
    key_created: bool,
    db_created: bool,
}

pub async fn run(config: &Config, nsec_file: Option<String>) -> Result<()> {
    let default_key_path = config
        .db_path
        .parent()
        .map(|p| p.join("identity.key"))
        .unwrap_or_else(|| PathBuf::from("identity.key"));

    let key_path = config.key_file.clone().unwrap_or(default_key_path);

    let (keys, key_created) = if let Some(nsec_path) = nsec_file {
        let content = std::fs::read_to_string(&nsec_path)
            .with_context(|| format!("Failed to read nsec file: {}", nsec_path))?;
        let secret = parse_secret_key(content.trim())?;
        let keys = nostr_sdk::Keys::new(secret);
        save_keys(&keys, &key_path)?;
        (keys, true)
    } else if key_path.exists() {
        let content = std::fs::read_to_string(&key_path)
            .with_context(|| format!("Failed to read existing key file: {:?}", key_path))?;
        let secret = parse_secret_key(content.trim())?;
        (nostr_sdk::Keys::new(secret), false)
    } else {
        let keys = generate_keys();
        save_keys(&keys, &key_path)?;
        (keys, true)
    };

    let db_created = if !config.db_path.exists() {
        if let Some(parent) = config.db_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create database directory")?;
        }
        let _storage = MdkSqliteStorage::new_unencrypted(&config.db_path)
            .context("Failed to initialize MDK database")?;
        true
    } else {
        false
    };

    let save_config = Config {
        key_file: Some(key_path.clone()),
        db_path: config.db_path.clone(),
        relays: config.relays.clone(),
    };
    if let Err(e) = save_config.save() {
        tracing::warn!("Failed to save config: {}", e);
    }

    let output = InitOutput {
        pubkey: keys.public_key().to_hex(),
        npub: keys.public_key().to_bech32().unwrap_or_default(),
        key_file: key_path.to_string_lossy().to_string(),
        db_path: config.db_path.to_string_lossy().to_string(),
        key_created,
        db_created,
    };

    print_json(output);
    Ok(())
}
