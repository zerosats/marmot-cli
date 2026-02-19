use anyhow::{Context, Result};
use mdk_core::messages::MessageProcessingResult;
use nostr_sdk::prelude::*;
use nostr_sdk::ToBech32;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
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
    last_event_id: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct CursorState {
    cursors: HashMap<String, u64>,
}

impl CursorState {
    fn path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".mdk").join("cursors.json"))
    }

    fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(&p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let Some(path) = Self::path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    fn get_cursor(&self, group_id: &str) -> Option<u64> {
        self.cursors.get(group_id).copied()
    }

    fn update_cursor(&mut self, group_id: &str, timestamp: u64) {
        let entry = self.cursors.entry(group_id.to_string()).or_insert(0);
        if timestamp > *entry {
            *entry = timestamp;
        }
    }
}

fn parse_since(since: &str) -> Option<u64> {
    if let Ok(ts) = since.parse::<u64>() {
        return Some(ts);
    }
    None
}

pub async fn run(
    config: &Config,
    group_id: Option<&str>,
    since: Option<&str>,
    watch: bool,
    poll_interval: u64,
) -> Result<()> {
    if watch {
        return run_watch(config, group_id, poll_interval).await;
    }

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
            last_event_id: None,
        };
        print_json(output);
        return Ok(());
    }

    let mut cursor_state = CursorState::load();

    let since_ts = since.and_then(parse_since);

    let tag_h = SingleLetterTag::lowercase(Alphabet::H);
    let mut filter = Filter::new()
        .kind(Kind::Custom(KIND_MLS_MESSAGE))
        .custom_tags(tag_h, group_ids_hex.iter().map(|s| s.as_str()))
        .limit(100);

    if let Some(ts) = since_ts {
        filter = filter.since(Timestamp::from_secs(ts));
    } else {
        let min_cursor = group_ids_hex.iter()
            .filter_map(|gid| cursor_state.get_cursor(gid))
            .min();
        if let Some(ts) = min_cursor {
            filter = filter.since(Timestamp::from_secs(ts));
        }
    }

    let events = nostr
        .fetch_events(filter, Duration::from_secs(FETCH_TIMEOUT_SECS))
        .await
        .context("Failed to fetch MLS messages")?;

    nostr.disconnect().await;

    let my_pubkey = ctx.pubkey();
    let mut messages: Vec<MessageInfo> = Vec::new();

    for event in &events {
        if event.pubkey == my_pubkey {
            continue;
        }

        match ctx.mdk.process_message(event) {
            Ok(result) => {
                if let MessageProcessingResult::ApplicationMessage(msg) = result {
                    let group_id_hex = hex::encode(msg.mls_group_id.as_slice());
                    cursor_state.update_cursor(&group_id_hex, event.created_at.as_secs());
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

    cursor_state.save();

    messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let last_event_id = messages.last().map(|m| m.event_id.clone());
    let count = messages.len();
    let output = ReceiveOutput { messages, count, last_event_id };

    print_json(output);
    Ok(())
}

async fn run_watch(config: &Config, group_id: Option<&str>, poll_interval: u64) -> Result<()> {
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
        anyhow::bail!("No groups to watch. Join a group first.");
    }

    let mut cursor_state = CursorState::load();
    let my_pubkey = ctx.pubkey();
    let stdout = std::io::stdout();

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });

    loop {
        let tag_h = SingleLetterTag::lowercase(Alphabet::H);
        let mut filter = Filter::new()
            .kind(Kind::Custom(KIND_MLS_MESSAGE))
            .custom_tags(tag_h, group_ids_hex.iter().map(|s| s.as_str()))
            .limit(100);

        let min_cursor = group_ids_hex.iter()
            .filter_map(|gid| cursor_state.get_cursor(gid))
            .min();
        if let Some(ts) = min_cursor {
            filter = filter.since(Timestamp::from_secs(ts));
        }

        if let Ok(events) = nostr
            .fetch_events(filter, Duration::from_secs(FETCH_TIMEOUT_SECS))
            .await
        {
            for event in &events {
                if event.pubkey == my_pubkey {
                    continue;
                }

                match ctx.mdk.process_message(event) {
                    Ok(result) => {
                        if let MessageProcessingResult::ApplicationMessage(msg) = result {
                            let group_id_hex = hex::encode(msg.mls_group_id.as_slice());
                            cursor_state.update_cursor(&group_id_hex, event.created_at.as_secs());
                            let info = MessageInfo {
                                event_id: event.id.to_hex(),
                                from_pubkey: msg.pubkey.to_hex(),
                                from_npub: msg.pubkey.to_bech32().unwrap_or_default(),
                                group_id: group_id_hex,
                                content: msg.content,
                                created_at: event.created_at.as_secs(),
                            };
                            if let Ok(json) = serde_json::to_string(&info) {
                                let mut handle = stdout.lock();
                                let _ = writeln!(handle, "{}", json);
                                let _ = handle.flush();
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
            cursor_state.save();
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(poll_interval)) => {},
            _ = shutdown_rx.changed() => {
                break;
            }
        }
    }

    nostr.disconnect().await;
    Ok(())
}
