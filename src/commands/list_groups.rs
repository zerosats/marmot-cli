use anyhow::{Context, Result};
use serde::Serialize;

use crate::config::Config;
use crate::mdk_helper::MdkContext;
use crate::output::print_json;

#[derive(Serialize)]
struct GroupInfo {
    nostr_group_id: String,
    name: String,
}

#[derive(Serialize)]
struct ListGroupsOutput {
    groups: Vec<GroupInfo>,
    count: usize,
}

pub async fn run(config: &Config) -> Result<()> {
    let ctx = MdkContext::load(config)?;

    let groups = ctx
        .mdk
        .get_groups()
        .context("Failed to get groups from MDK")?;

    let group_infos: Vec<GroupInfo> = groups
        .into_iter()
        .map(|g| GroupInfo {
            nostr_group_id: hex::encode(g.nostr_group_id),
            name: g.name,
        })
        .collect();

    let count = group_infos.len();
    let output = ListGroupsOutput {
        groups: group_infos,
        count,
    };

    print_json(output);
    Ok(())
}
