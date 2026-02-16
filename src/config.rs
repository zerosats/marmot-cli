use anyhow::{Context, Result};
use std::path::PathBuf;

pub struct Config {
    pub key_file: Option<PathBuf>,
    pub db_path: PathBuf,
    pub relays: Vec<String>,
}

impl Config {
    pub fn load(cli: &crate::Cli) -> Result<Self> {
        // Determine database path
        let db_path = match &cli.db_path {
            Some(p) => PathBuf::from(p),
            None => {
                let home = dirs::home_dir().context("Could not determine home directory")?;
                home.join(".mdk").join("state.db")
            }
        };

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create database directory")?;
        }

        // Determine key file path
        let key_file = cli.key_file.as_ref().map(PathBuf::from);

        // Determine relays
        let relays = cli.relays.clone().unwrap_or_else(|| {
            vec![
                "wss://relay.primal.net".to_string(),
                "wss://relay.damus.io".to_string(),
            ]
        });

        Ok(Self {
            key_file,
            db_path,
            relays,
        })
    }

    pub fn load_nsec(&self) -> Result<String> {
        let key_file = self.key_file.as_ref()
            .context("No key file specified. Use --key-file or set MDK_KEY_FILE")?;

        let content = std::fs::read_to_string(key_file)
            .context("Failed to read key file")?;

        Ok(content.trim().to_string())
    }
}
