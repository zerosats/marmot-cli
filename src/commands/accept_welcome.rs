use anyhow::{bail, Context, Result};
use nostr_sdk::nips::nip59;
use nostr_sdk::prelude::*;
use serde::Serialize;
use std::time::Duration;

use crate::config::Config;
use crate::mdk_helper::MdkContext;
use crate::nostr_client::NostrClient;
use crate::output::print_json;

const KIND_WELCOME: u16 = 444;
const KIND_GIFT_WRAP: u16 = 1059;
const FETCH_TIMEOUT_SECS: u64 = 10;

#[derive(Serialize)]
struct AcceptOutput {
    nostr_group_id: String,
    group_name: String,
    member_count: u32,
    event_id: String,
}

pub async fn run(config: &Config, event_id: &str) -> Result<()> {
    let ctx = MdkContext::load(config)?;
    let nostr = NostrClient::new(&ctx.keys, config.relays.clone()).await?;

    let event_id_parsed = EventId::from_hex(event_id)
        .or_else(|_| EventId::from_bech32(event_id))
        .context("Invalid event ID format")?;

    let filter = Filter::new()
        .id(event_id_parsed)
        .limit(1);

    let events = nostr
        .fetch_events(filter, Duration::from_secs(FETCH_TIMEOUT_SECS))
        .await
        .context("Failed to fetch welcome event")?;

    nostr.disconnect().await;

    let event = events
        .into_iter()
        .next()
        .with_context(|| format!("Event not found: {}", event_id))?;

    let kind = event.kind.as_u16();

    let (rumor, _sender) = if kind == KIND_GIFT_WRAP {
        let unwrapped = nip59::extract_rumor(&ctx.keys, &event)
            .await
            .context("Failed to unwrap gift-wrap")?;

        if unwrapped.rumor.kind.as_u16() != KIND_WELCOME {
            bail!(
                "Gift-wrapped event contains kind {}, expected {}",
                unwrapped.rumor.kind.as_u16(),
                KIND_WELCOME
            );
        }

        (unwrapped.rumor, unwrapped.sender)
    } else if kind == KIND_WELCOME {
        let rumor: UnsignedEvent = serde_json::from_str(&event.content)
            .context("Failed to parse welcome rumor from event content")?;
        (rumor, event.pubkey)
    } else {
        bail!("Event kind {} is not a welcome (444) or gift-wrap (1059)", kind);
    };

    let welcome = ctx
        .mdk
        .process_welcome(&event.id, &rumor)
        .context("Failed to process MLS welcome")?;

    ctx.mdk
        .accept_welcome(&welcome)
        .context("Failed to accept MLS welcome")?;

    let output = AcceptOutput {
        nostr_group_id: hex::encode(&welcome.nostr_group_id),
        group_name: welcome.group_name,
        member_count: welcome.member_count,
        event_id: event.id.to_hex(),
    };

    print_json(output);
    Ok(())
}
