use anyhow::{Context, Result};
use nostr_sdk::prelude::*;
use serde::Serialize;
use std::time::Duration;

use crate::config::Config;
use crate::mdk_helper::MdkContext;
use crate::nostr_client::NostrClient;
use crate::output::print_json;

#[derive(Serialize)]
struct SendOutput {
    event_id: String,
    group_id: String,
    message_length: usize,
}

pub async fn run(config: &Config, group_id: &str, message: &str) -> Result<()> {
    let ctx = MdkContext::load(config)?;

    let nostr_group_id_bytes: [u8; 32] = hex::decode(group_id)
        .context("Invalid group ID hex")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Group ID must be 32 bytes"))?;

    let groups = ctx
        .mdk
        .get_groups()
        .context("Failed to get groups")?;

    let group = groups
        .iter()
        .find(|g| g.nostr_group_id == nostr_group_id_bytes)
        .with_context(|| format!("Group not found: {}", group_id))?;

    let mls_group_id = group.mls_group_id.clone();

    let rumor = EventBuilder::new(Kind::Custom(9), message)
        .build(ctx.pubkey());

    let event = ctx
        .mdk
        .create_message(&mls_group_id, rumor)
        .context("Failed to create MLS encrypted message")?;

    let nostr = NostrClient::new(&ctx.keys, config.relays.clone()).await?;
    let event_id = nostr.publish(event).await?;

    tokio::time::sleep(Duration::from_millis(500)).await;
    nostr.disconnect().await;

    let output = SendOutput {
        event_id: event_id.to_hex(),
        group_id: group_id.to_string(),
        message_length: message.len(),
    };

    print_json(output);
    Ok(())
}
