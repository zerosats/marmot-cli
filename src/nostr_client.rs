use anyhow::Result;
use nostr_sdk::prelude::*;
use std::time::Duration;

pub struct NostrClient {
    client: Client,
    relays: Vec<String>,
}

impl NostrClient {
    pub async fn new(keys: &Keys, relays: Vec<String>) -> Result<Self> {
        let client = Client::new(keys.clone());

        for relay in &relays {
            client.add_relay(relay).await?;
        }

        client.connect().await;

        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(Self { client, relays })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn relays(&self) -> &[String] {
        &self.relays
    }

    pub async fn publish(&self, event: Event) -> Result<EventId> {
        let output = self.client.send_event(&event).await?;
        Ok(output.id().clone())
    }

    pub async fn fetch_events(&self, filter: Filter, timeout: Duration) -> Result<Vec<Event>> {
        let events = self.client
            .fetch_events(filter, timeout)
            .await?;
        Ok(events.into_iter().collect())
    }

    pub async fn disconnect(&self) {
        self.client.disconnect().await;
    }
}
