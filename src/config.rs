use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
struct ConfigFile {
    key_file: Option<String>,
    db_path: Option<String>,
    relays: Option<Vec<String>>,
}

pub struct Config {
    pub key_file: Option<PathBuf>,
    pub db_path: PathBuf,
    pub relays: Vec<String>,
}

impl Config {
    fn config_file_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".mdk").join("config.toml"))
    }

    fn load_config_file() -> ConfigFile {
        Self::config_file_path()
            .and_then(|p| std::fs::read_to_string(&p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn load(cli: &crate::Cli) -> Result<Self> {
        let file_config = Self::load_config_file();

        let db_path = match &cli.db_path {
            Some(p) => PathBuf::from(p),
            None => match file_config.db_path {
                Some(ref p) => PathBuf::from(p),
                None => {
                    let home = dirs::home_dir().context("Could not determine home directory")?;
                    home.join(".mdk").join("state.db")
                }
            },
        };

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create database directory")?;
        }

        let key_file = cli.key_file.as_ref().map(PathBuf::from)
            .or_else(|| file_config.key_file.map(PathBuf::from));

        let default_relays = || vec![
            "wss://relay.primal.net".to_string(),
            "wss://relay.damus.io".to_string(),
        ];

        let relays = cli.relays.clone()
            .or(file_config.relays)
            .unwrap_or_else(default_relays);

        Ok(Self {
            key_file,
            db_path,
            relays,
        })
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_file_path()
            .context("Could not determine home directory")?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }

        let file_config = ConfigFile {
            key_file: self.key_file.as_ref().map(|p| p.to_string_lossy().to_string()),
            db_path: Some(self.db_path.to_string_lossy().to_string()),
            relays: Some(self.relays.clone()),
        };

        let content = toml::to_string_pretty(&file_config)
            .context("Failed to serialize config")?;

        std::fs::write(&path, content)
            .context("Failed to write config file")?;

        Ok(())
    }

    pub fn load_nsec(&self) -> Result<String> {
        let key_file = self.key_file.as_ref()
            .context("No key file specified. Use --key-file or set MDK_KEY_FILE")?;

        let content = std::fs::read_to_string(key_file)
            .context("Failed to read key file")?;

        Ok(content.trim().to_string())
    }
}
