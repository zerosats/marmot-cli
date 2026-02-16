use anyhow::{Context, Result};
use nostr_sdk::nips::nip59;
use nostr_sdk::prelude::*;
use nostr_sdk::ToBech32;
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
struct WelcomeInfo {
    event_id: String,
    from_pubkey: String,
    from_npub: String,
    created_at: u64,
    is_gift_wrapped: bool,
}

#[derive(Serialize)]
struct ListWelcomesOutput {
    welcomes: Vec<WelcomeInfo>,
    count: usize,
}

pub async fn run(config: &Config) -> Result<()> {
    let ctx = MdkContext::load(config)?;
    let nostr = NostrClient::new(&ctx.keys, config.relays.clone()).await?;

    let pubkey = ctx.pubkey();

    let welcome_filter = Filter::new()
        .kind(Kind::Custom(KIND_WELCOME))
        .pubkey(pubkey)
        .limit(50);

    let gift_wrap_filter = Filter::new()
        .kind(Kind::Custom(KIND_GIFT_WRAP))
        .pubkey(pubkey)
        .limit(50);

    let welcome_events = nostr
        .fetch_events(welcome_filter, Duration::from_secs(FETCH_TIMEOUT_SECS))
        .await
        .context("Failed to fetch welcome events")?;

    let gift_wrap_events = nostr
        .fetch_events(gift_wrap_filter, Duration::from_secs(FETCH_TIMEOUT_SECS))
        .await
        .context("Failed to fetch gift-wrap events")?;

    nostr.disconnect().await;

    let mut welcomes: Vec<WelcomeInfo> = Vec::new();

    for event in welcome_events {
        welcomes.push(WelcomeInfo {
            event_id: event.id.to_hex(),
            from_pubkey: event.pubkey.to_hex(),
            from_npub: event.pubkey.to_bech32().unwrap_or_default(),
            created_at: event.created_at.as_secs(),
            is_gift_wrapped: false,
        });
    }

    for event in gift_wrap_events {
        match nip59::extract_rumor(&ctx.keys, &event).await {
            Ok(unwrapped) => {
                if unwrapped.rumor.kind.as_u16() == KIND_WELCOME {
                    welcomes.push(WelcomeInfo {
                        event_id: event.id.to_hex(),
                        from_pubkey: unwrapped.sender.to_hex(),
                        from_npub: unwrapped.sender.to_bech32().unwrap_or_default(),
                        created_at: event.created_at.as_secs(),
                        is_gift_wrapped: true,
                    });
                }
            }
            Err(_) => continue,
        }
    }

    welcomes.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let count = welcomes.len();
    let output = ListWelcomesOutput { welcomes, count };

    print_json(output);
    Ok(())
}
