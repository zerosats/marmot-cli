use anyhow::Result;
use serde::Serialize;

use crate::config::Config;
use crate::mdk_helper::MdkContext;
use crate::output::print_json;

#[derive(Serialize)]
struct WhoamiOutput {
    pubkey: String,
    npub: String,
    key_file: Option<String>,
    db_path: String,
    db_exists: bool,
    relays: Vec<String>,
}

pub async fn run(config: &Config) -> Result<()> {
    let ctx = MdkContext::load(config)?;

    let output = WhoamiOutput {
        pubkey: ctx.pubkey().to_hex(),
        npub: ctx.npub(),
        key_file: config.key_file.as_ref().map(|p| p.to_string_lossy().to_string()),
        db_path: config.db_path.to_string_lossy().to_string(),
        db_exists: config.db_path.exists(),
        relays: config.relays.clone(),
    };

    print_json(output);
    Ok(())
}
