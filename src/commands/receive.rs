use anyhow::{Context, Result};
use mdk_core::messages::MessageProcessingResult;
use nostr_sdk::prelude::*;
use nostr_sdk::ToBech32;
use serde::Serialize;
use std::time::Duration;

use crate::config::Config;
use crate::mdk_helper::MdkContext;
use crate::nostr_client::NostrClient;
use crate::output::print_json;

const KIND_MLS_MESSAGE: u16 = 445;
const FETCH_TIMEOUT_SECS: u64 = 10;

#[derive(Serialize)]
struct MessageInfo {
    event_id: String,
    from_pubkey: String,
    from_npub: String,
    group_id: String,
    content: String,
    created_at: u64,
}

#[derive(Serialize)]
struct ReceiveOutput {
    messages: Vec<MessageInfo>,
    count: usize,
}

pub async fn run(config: &Config, group_id: Option<&str>) -> Result<()> {
    let ctx = MdkContext::load(config)?;
    let nostr = NostrClient::new(&ctx.keys, config.relays.clone()).await?;

    let groups = ctx
        .mdk
        .get_groups()
        .context("Failed to get groups")?;

    let group_ids_hex: Vec<String> = if let Some(gid) = group_id {
        vec![gid.to_string()]
    } else {
        groups
            .iter()
            .map(|g| hex::encode(&g.nostr_group_id))
            .collect()
    };

    if group_ids_hex.is_empty() {
        let output = ReceiveOutput {
            messages: vec![],
            count: 0,
        };
        print_json(output);
        return Ok(());
    }

    let tag_h = SingleLetterTag::lowercase(Alphabet::H);
    let filter = Filter::new()
        .kind(Kind::Custom(KIND_MLS_MESSAGE))
        .custom_tags(tag_h, group_ids_hex.iter().map(|s| s.as_str()))
        .limit(100);

    let events = nostr
        .fetch_events(filter, Duration::from_secs(FETCH_TIMEOUT_SECS))
        .await
        .context("Failed to fetch MLS messages")?;

    nostr.disconnect().await;

    let my_pubkey = ctx.pubkey();
    let mut messages: Vec<MessageInfo> = Vec::new();

    for event in events {
        if event.pubkey == my_pubkey {
            continue;
        }

        match ctx.mdk.process_message(&event) {
            Ok(result) => {
                if let MessageProcessingResult::ApplicationMessage(msg) = result {
                    let group_id_hex = hex::encode(msg.mls_group_id.as_slice());
                    messages.push(MessageInfo {
                        event_id: event.id.to_hex(),
                        from_pubkey: msg.pubkey.to_hex(),
                        from_npub: msg.pubkey.to_bech32().unwrap_or_default(),
                        group_id: group_id_hex,
                        content: msg.content,
                        created_at: event.created_at.as_secs(),
                    });
                }
            }
            Err(_) => continue,
        }
    }

    messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let count = messages.len();
    let output = ReceiveOutput { messages, count };

    print_json(output);
    Ok(())
}
