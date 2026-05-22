//! Codex / logos-storage REST client implementing `indexing::StorageClient`.
//!
//! Wraps the upload path in an exponential-backoff retry loop so
//! transient 5xx / connection-reset failures don't drop the user's
//! file. See LP-0017 §Reliability R10.

use async_trait::async_trait;
use indexing::{with_retry, IndexingError, IndexingResult, RetryConfig, StorageClient};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct CodexRest {
    base: String,
    http: reqwest::Client,
    retry: RetryConfig,
}

impl CodexRest {
    pub fn new(url: impl Into<String>) -> Self {
        Self::with_client(url, reqwest::Client::new())
    }

    pub fn with_client(url: impl Into<String>, http: reqwest::Client) -> Self {
        Self {
            base: url.into().trim_end_matches('/').to_string(),
            http,
            retry: RetryConfig::default(),
        }
    }

    /// Override the retry policy. Mostly useful for tests that need a
    /// zero-delay config so they don't sleep through assertions.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    fn endpoint(&self, p: &str) -> String {
        format!("{}{}", self.base, p)
    }

    async fn post_once(&self, filename: &str, bytes: &[u8]) -> IndexingResult<String> {
        let r = self
            .http
            .post(self.endpoint("/api/storage/v1/data"))
            .header("Content-Type", "application/octet-stream")
            .header(
                "Content-Disposition",
                format!(r#"attachment; filename="{}""#, sanitize_filename(filename)),
            )
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|e| IndexingError::Transport(e.to_string()))?;
        let status = r.status();
        if !status.is_success() {
            let body = r.text().await.unwrap_or_default();
            // 4xx is a permanent error — retrying won't fix a bad
            // request. 5xx + 408 (timeout) + 429 (rate limit) are the
            // retry-worth classes.
            if status.is_server_error() || status.as_u16() == 408 || status.as_u16() == 429 {
                return Err(IndexingError::Transport(format!("upload {status}: {body}")));
            }
            return Err(IndexingError::Backend(format!("upload {status}: {body}")));
        }
        let cid = r
            .text()
            .await
            .map_err(|e| IndexingError::Transport(e.to_string()))?;
        let cid = cid.trim().to_string();
        if cid.is_empty() {
            return Err(IndexingError::Unexpected(
                "storage returned empty CID".into(),
            ));
        }
        Ok(cid)
    }

    async fn post_bytes(&self, filename: &str, bytes: Vec<u8>) -> IndexingResult<String> {
        // Wrap in Arc so the retry closure can borrow shared, owned
        // bytes across attempts without re-cloning the buffer every
        // single retry.
        let bytes = Arc::new(bytes);
        let filename = filename.to_string();
        with_retry(self.retry, || {
            let bytes = Arc::clone(&bytes);
            let filename = filename.clone();
            async move { self.post_once(&filename, &bytes).await }
        })
        .await
        .map_err(|e: IndexingError| match e {
            // After exhausting retries, re-shape Transport into a Backend
            // error so the caller sees "retries exhausted" rather than a
            // raw IO message.
            IndexingError::Transport(s) => {
                IndexingError::Backend(format!("upload retries exhausted: {s}"))
            }
            other => other,
        })
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '"' | '\\' | '\n' | '\r' => '_',
            other => other,
        })
        .collect()
}

#[async_trait]
impl StorageClient for CodexRest {
    async fn healthy(&self) -> bool {
        // Logos Storage doesn't ship a dedicated /health endpoint.
        // `GET /api/storage/v1/spr` returns 200 with the node's
        // Signed Peer Record once the daemon is fully up — perfect
        // "fully booted" signal that doesn't require a known CID.
        match self
            .http
            .get(self.endpoint("/api/storage/v1/spr"))
            .send()
            .await
        {
            Ok(r) => r.status().is_success(),
            Err(_) => false,
        }
    }

    async fn upload_file(&self, path: &Path) -> IndexingResult<String> {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| IndexingError::Transport(format!("read {}: {e}", path.display())))?;
        let fname = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "upload.bin".to_string());
        self.post_bytes(&fname, bytes).await
    }

    async fn upload_bytes(&self, filename: &str, bytes: &[u8]) -> IndexingResult<String> {
        self.post_bytes(filename, bytes.to_vec()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn endpoint_normalises_trailing_slash() {
        let c = CodexRest::new("http://localhost:18080/");
        assert_eq!(
            c.endpoint("/api/storage/v1/data"),
            "http://localhost:18080/api/storage/v1/data"
        );
    }

    #[test]
    fn sanitize_filename_drops_quotes_and_newlines() {
        assert_eq!(sanitize_filename(r#"a"b\c"#), "a_b_c");
        assert_eq!(sanitize_filename("safe.pdf"), "safe.pdf");
        assert_eq!(sanitize_filename("multi\nline"), "multi_line");
    }

    #[tokio::test]
    async fn upload_exhausts_retries_against_unreachable_endpoint() {
        // Pick an unrouteable port; reqwest fails fast.
        let c = CodexRest::new("http://127.0.0.1:1").with_retry(RetryConfig {
            attempts: 3,
            base_delay: Duration::ZERO,
            growth: 1.0,
            max_delay: Duration::ZERO,
        });
        let err = c.upload_bytes("x.bin", &[0u8; 4]).await.unwrap_err();
        // After the retry loop exhausts, we re-shape Transport into a
        // Backend error so the caller can distinguish "give up" from
        // "still retrying."
        match err {
            IndexingError::Backend(msg) => {
                assert!(
                    msg.contains("retries exhausted"),
                    "expected retries-exhausted message, got: {msg}"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
