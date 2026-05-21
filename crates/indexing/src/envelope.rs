//! The canonical Delivery-topic envelope.
//!
//! Wire format is locked at v1 in `docs/decisions/002-envelope-schema.md`.
//! The schema is shared verbatim with Thompson's chronicle module so a
//! single batch-anchor tool can consume both
//! `/whistleblower/1/document-broadcast/json` and
//! `/chronicle/1/document-index/json`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Parsing errors. Anything other than `MissingField` / `BadVersion` /
/// `BadHash` indicates a malformed payload and the batch-anchor CLI
/// silently drops the envelope.
#[derive(Debug, Error)]
pub enum EnvelopeError {
    /// JSON parse failure.
    #[error("invalid json: {0}")]
    Json(String),

    /// Base64 decode failure on the Waku payload.
    #[error("invalid base64: {0}")]
    Base64(String),

    /// `v` is missing or != 1.
    #[error("unsupported version (expected v=1)")]
    BadVersion,

    /// `metadata_hash` is not `v1:<64 hex chars>`.
    #[error("bad metadata_hash: {0}")]
    BadHash(String),

    /// `cid` empty, `timestamp` zero, or required field absent.
    #[error("missing or empty required field: {0}")]
    MissingField(&'static str),
}

/// The envelope as it sits in `payload` (after base64 decode + UTF-8 +
/// JSON parse).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    /// Schema version. Always `1` at v1.
    pub v: u8,
    /// Multiformat CID — opaque string, dedup key.
    pub cid: String,
    /// `v1:<sha256 hex>` over the canonical metadata JSON (see [`canonical_metadata_hash`]).
    pub metadata_hash: String,
    /// Unix seconds.
    pub timestamp: u64,

    /// Optional discovery metadata — anchor pipeline ignores all of these.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional discovery metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional discovery metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Optional discovery metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Optional discovery metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Envelope {
    /// Decode the 32-byte `metadata_hash` after stripping the `v1:` prefix.
    pub fn metadata_hash_bytes(&self) -> Result<[u8; 32], EnvelopeError> {
        let hex_str = self
            .metadata_hash
            .strip_prefix("v1:")
            .ok_or_else(|| EnvelopeError::BadHash("missing v1: prefix".into()))?;
        let bytes = hex::decode(hex_str)
            .map_err(|e| EnvelopeError::BadHash(format!("hex decode: {e}")))?;
        bytes
            .try_into()
            .map_err(|_| EnvelopeError::BadHash("hash is not 32 bytes".into()))
    }

    /// Validate required fields. Returns `Ok(())` if the envelope can be
    /// passed to `RegistryClient::index_batch`.
    pub fn validate(&self) -> Result<(), EnvelopeError> {
        if self.v != 1 {
            return Err(EnvelopeError::BadVersion);
        }
        if self.cid.is_empty() {
            return Err(EnvelopeError::MissingField("cid"));
        }
        if self.timestamp == 0 {
            return Err(EnvelopeError::MissingField("timestamp"));
        }
        self.metadata_hash_bytes()?;
        Ok(())
    }

    /// Parse from a base64-encoded JSON Waku payload.
    pub fn from_base64_payload(b64: &str) -> Result<Self, EnvelopeError> {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let bytes = B64
            .decode(b64.trim())
            .map_err(|e| EnvelopeError::Base64(e.to_string()))?;
        let env: Envelope =
            serde_json::from_slice(&bytes).map_err(|e| EnvelopeError::Json(e.to_string()))?;
        env.validate()?;
        Ok(env)
    }
}

/// Compute `metadata_hash` over the canonical discovery-metadata JSON.
///
/// Canonical form (per ADR-002):
/// ```text
/// { "title": ..., "description": ..., "content_type": ..., "size_bytes": ..., "tags": [...] }
/// ```
/// Keys appear in this exact order. Keys whose value is `None` or whose
/// `tags` is empty are omitted.
pub fn canonical_metadata_hash(
    title: Option<&str>,
    description: Option<&str>,
    content_type: Option<&str>,
    size_bytes: Option<u64>,
    tags: &[String],
) -> String {
    // Build the canonical JSON manually — we cannot rely on serde_json's
    // map ordering, and we need byte-stable output across implementations.
    let mut parts: Vec<String> = Vec::new();
    if let Some(t) = title {
        parts.push(format!("\"title\":{}", json_string(t)));
    }
    if let Some(d) = description {
        parts.push(format!("\"description\":{}", json_string(d)));
    }
    if let Some(ct) = content_type {
        parts.push(format!("\"content_type\":{}", json_string(ct)));
    }
    if let Some(sz) = size_bytes {
        parts.push(format!("\"size_bytes\":{sz}"));
    }
    if !tags.is_empty() {
        let arr = tags
            .iter()
            .map(|t| json_string(t))
            .collect::<Vec<_>>()
            .join(",");
        parts.push(format!("\"tags\":[{arr}]"));
    }
    let canonical = format!("{{{}}}", parts.join(","));
    let hash = Sha256::digest(canonical.as_bytes());
    format!("v1:{}", hex::encode(hash))
}

fn json_string(s: &str) -> String {
    // Minimal JSON-string escape: " \ control chars. Anything fancier
    // would diverge from RFC 8259 — keep it strict.
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as B64, Engine};

    fn sample() -> Envelope {
        Envelope {
            v: 1,
            cid: "zDvZRwzk".into(),
            metadata_hash: format!("v1:{}", "a".repeat(64)),
            timestamp: 1_700_000_000,
            title: Some("leak.pdf".into()),
            description: None,
            content_type: Some("application/pdf".into()),
            size_bytes: Some(12345),
            tags: vec!["leak".into(), "internal".into()],
        }
    }

    #[test]
    fn validate_happy_path() {
        sample().validate().unwrap();
    }

    #[test]
    fn rejects_v_not_1() {
        let mut e = sample();
        e.v = 2;
        assert!(matches!(e.validate(), Err(EnvelopeError::BadVersion)));
    }

    #[test]
    fn rejects_empty_cid() {
        let mut e = sample();
        e.cid = "".into();
        assert!(matches!(e.validate(), Err(EnvelopeError::MissingField("cid"))));
    }

    #[test]
    fn rejects_zero_timestamp() {
        let mut e = sample();
        e.timestamp = 0;
        assert!(matches!(e.validate(), Err(EnvelopeError::MissingField("timestamp"))));
    }

    #[test]
    fn rejects_bad_metadata_hash() {
        let mut e = sample();
        e.metadata_hash = "garbage".into();
        assert!(matches!(e.validate(), Err(EnvelopeError::BadHash(_))));
    }

    #[test]
    fn metadata_hash_bytes_roundtrip() {
        let e = sample();
        let bytes = e.metadata_hash_bytes().unwrap();
        assert_eq!(bytes, [0xaa; 32]);
    }

    #[test]
    fn from_base64_payload_roundtrip() {
        let env = sample();
        let bytes = serde_json::to_vec(&env).unwrap();
        let b64 = B64.encode(&bytes);
        let back = Envelope::from_base64_payload(&b64).unwrap();
        assert_eq!(back, env);
    }

    #[test]
    fn canonical_hash_is_stable() {
        let a = canonical_metadata_hash(
            Some("t"),
            Some("d"),
            Some("text/plain"),
            Some(10),
            &["x".into(), "y".into()],
        );
        let b = canonical_metadata_hash(
            Some("t"),
            Some("d"),
            Some("text/plain"),
            Some(10),
            &["x".into(), "y".into()],
        );
        assert_eq!(a, b);
        assert!(a.starts_with("v1:"));
        assert_eq!(a.len(), 67); // "v1:" + 64 hex chars.
    }

    #[test]
    fn canonical_hash_omits_empty_fields() {
        // Only `title` present -> single-key canonical object.
        let h = canonical_metadata_hash(Some("only"), None, None, None, &[]);
        // Recompute manually:
        let expected = {
            let canon = r#"{"title":"only"}"#;
            let hash = Sha256::digest(canon.as_bytes());
            format!("v1:{}", hex::encode(hash))
        };
        assert_eq!(h, expected);
    }

    #[test]
    fn canonical_hash_changes_with_tags() {
        let a = canonical_metadata_hash(Some("t"), None, None, None, &[]);
        let b = canonical_metadata_hash(Some("t"), None, None, None, &["one".into()]);
        assert_ne!(a, b);
    }

    #[test]
    fn json_string_escapes_quotes_and_backslashes() {
        assert_eq!(json_string(r#"hello "world""#), r#""hello \"world\"""#);
        assert_eq!(json_string("c:\\path"), r#""c:\\path""#);
        assert_eq!(json_string("line\nbreak"), r#""line\nbreak""#);
    }
}
