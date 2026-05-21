//! Real nwaku REST client implementing `indexing::DeliveryClient`.
//!
//! No mocks. Talks to a running nwaku v0.38.x node at the configured
//! REST endpoint. See `infra/docker-compose.yml` for a compose file
//! that brings up a node compatible with the `logos.dev` cluster.
//!
//! Endpoints exercised:
//! - `GET /health`
//! - `POST /relay/v1/auto/subscriptions`
//! - `POST /relay/v1/auto/messages`
//! - `GET  /relay/v1/auto/messages/{urlencoded-topic}` (destructive drain)
//! - `GET  /store/v3/messages?...` (paginated catch-up)

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use indexing::{DeliveryClient, Envelope, IndexingError, IndexingResult};
use serde::Deserialize;

/// nwaku REST client. Clones are cheap (the inner [`reqwest::Client`] is
/// shareable).
#[derive(Clone, Debug)]
pub struct NwakuRest {
    base: String,
    http: reqwest::Client,
}

impl NwakuRest {
    pub fn new(url: impl Into<String>) -> Self {
        Self::with_client(url, reqwest::Client::new())
    }

    pub fn with_client(url: impl Into<String>, http: reqwest::Client) -> Self {
        Self {
            base: url.into().trim_end_matches('/').to_string(),
            http,
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }
}

#[derive(Deserialize)]
struct WakuRelayMessage {
    payload: String,
    #[serde(default, rename = "contentTopic")]
    _content_topic: String,
}

#[derive(Deserialize)]
struct StoreResponse {
    #[serde(default)]
    messages: Vec<StoreMessage>,
    #[serde(default)]
    pagination_cursor: Option<String>,
}

#[derive(Deserialize)]
struct StoreMessage {
    #[serde(default)]
    payload: Option<String>,
    #[serde(default)]
    message: Option<EmbeddedMessage>,
}

#[derive(Deserialize)]
struct EmbeddedMessage {
    payload: Option<String>,
}

impl StoreMessage {
    fn payload_b64(&self) -> Option<&str> {
        self.payload
            .as_deref()
            .or_else(|| self.message.as_ref().and_then(|m| m.payload.as_deref()))
    }
}

fn to_io(e: impl std::fmt::Display) -> IndexingError {
    IndexingError::Transport(e.to_string())
}

#[async_trait]
impl DeliveryClient for NwakuRest {
    async fn healthy(&self) -> bool {
        match self.http.get(self.endpoint("/health")).send().await {
            Ok(r) => r.status().is_success(),
            Err(_) => false,
        }
    }

    async fn subscribe(&self, topic: &str) -> IndexingResult<()> {
        let r = self
            .http
            .post(self.endpoint("/relay/v1/auto/subscriptions"))
            .json(&serde_json::json!([topic]))
            .send()
            .await
            .map_err(to_io)?;
        let status = r.status();
        // nwaku returns either an empty 200 or `"OK"` as a JSON string.
        // Anything else is an actual error.
        let body = r.text().await.unwrap_or_default();
        if status.is_success() || body.trim().trim_matches('"').eq_ignore_ascii_case("ok") {
            Ok(())
        } else {
            Err(IndexingError::Backend(format!(
                "subscribe {status}: {body}"
            )))
        }
    }

    async fn publish(&self, topic: &str, env: &Envelope) -> IndexingResult<()> {
        let body = serde_json::to_vec(env).map_err(to_io)?;
        let payload_b64 = B64.encode(&body);
        let r = self
            .http
            .post(self.endpoint("/relay/v1/auto/messages"))
            .json(&serde_json::json!({
                "contentTopic": topic,
                "payload": payload_b64,
            }))
            .send()
            .await
            .map_err(to_io)?;
        if r.status().is_success() {
            Ok(())
        } else {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            Err(IndexingError::Backend(format!("publish {status}: {body}")))
        }
    }

    async fn drain(&self, topic: &str) -> IndexingResult<Vec<Envelope>> {
        let enc = urlencoding::encode(topic).into_owned();
        let r = self
            .http
            .get(self.endpoint(&format!("/relay/v1/auto/messages/{enc}")))
            .send()
            .await
            .map_err(to_io)?;
        // 404 from nwaku means the queue is empty since the last drain
        // — that's a steady state, not an error.
        if r.status().as_u16() == 404 {
            return Ok(vec![]);
        }
        if !r.status().is_success() {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            return Err(IndexingError::Backend(format!("drain {status}: {body}")));
        }
        let msgs: Vec<WakuRelayMessage> = r.json().await.map_err(to_io)?;
        Ok(msgs
            .into_iter()
            .filter_map(|m| Envelope::from_base64_payload(&m.payload).ok())
            .collect())
    }

    async fn query_store(&self, topic: &str, start_ns: u128) -> IndexingResult<Vec<Envelope>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        let enc_topic = urlencoding::encode(topic).into_owned();
        loop {
            let mut url = format!(
                "{}/store/v3/messages?contentTopics={enc_topic}&pageSize=100&includeData=true&ascending=true",
                self.base
            );
            if start_ns > 0 {
                url.push_str(&format!("&startTime={start_ns}"));
            }
            if let Some(c) = &cursor {
                url.push_str(&format!("&cursor={}", urlencoding::encode(c)));
            }
            let r = self.http.get(&url).send().await.map_err(to_io)?;
            if !r.status().is_success() {
                let status = r.status();
                let body = r.text().await.unwrap_or_default();
                return Err(IndexingError::Backend(format!(
                    "store query {status}: {body}"
                )));
            }
            let resp: StoreResponse = r.json().await.map_err(to_io)?;
            for m in &resp.messages {
                if let Some(p) = m.payload_b64() {
                    if let Ok(env) = Envelope::from_base64_payload(p) {
                        out.push(env);
                    }
                }
            }
            match resp.pagination_cursor {
                Some(c) if !c.is_empty() && resp.messages.len() == 100 => cursor = Some(c),
                _ => break,
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_trims_trailing_slash() {
        let c = NwakuRest::new("http://nwaku:8645/");
        assert_eq!(c.endpoint("/health"), "http://nwaku:8645/health");
    }

    #[test]
    fn store_message_prefers_top_level_payload() {
        let m = StoreMessage {
            payload: Some("top".into()),
            message: Some(EmbeddedMessage {
                payload: Some("embedded".into()),
            }),
        };
        assert_eq!(m.payload_b64(), Some("top"));
    }

    #[test]
    fn store_message_falls_back_to_embedded_payload() {
        let m = StoreMessage {
            payload: None,
            message: Some(EmbeddedMessage {
                payload: Some("embedded".into()),
            }),
        };
        assert_eq!(m.payload_b64(), Some("embedded"));
    }

    #[test]
    fn store_message_returns_none_when_both_absent() {
        let m = StoreMessage {
            payload: None,
            message: None,
        };
        assert_eq!(m.payload_b64(), None);
    }
}
