use anyhow::{Context, Result};
use nostr_sdk::prelude::*;
use serde::Serialize;
use std::time::Duration;

use crate::config::Config;
use crate::mdk_helper::MdkContext;
use crate::nostr_client::NostrClient;
use crate::output::print_json;

#[derive(Serialize)]
struct PublishOutput {
    event_id: String,
    pubkey: String,
    relays: Vec<String>,
}

pub async fn run(config: &Config) -> Result<()> {
    let ctx = MdkContext::load(config)?;

    let (content, tags, _key_package_id) = ctx
        .mdk
        .create_key_package_for_event(&ctx.pubkey(), ctx.relays.clone())
        .context("Failed to create MLS key package")?;

    let mut builder = EventBuilder::new(Kind::MlsKeyPackage, content);
    for tag in tags {
        builder = builder.tag(tag);
    }

    let event = builder
        .sign(&ctx.keys)
        .await
        .context("Failed to sign key package event")?;

    let nostr = NostrClient::new(&ctx.keys, config.relays.clone()).await?;
    let event_id = nostr.publish(event).await?;

    tokio::time::sleep(Duration::from_millis(500)).await;
    nostr.disconnect().await;

    let output = PublishOutput {
        event_id: event_id.to_hex(),
        pubkey: ctx.pubkey().to_hex(),
        relays: config.relays.clone(),
    };

    print_json(output);
    Ok(())
}
