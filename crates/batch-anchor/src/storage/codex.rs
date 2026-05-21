//! Codex / logos-storage REST client implementing `indexing::StorageClient`.

use async_trait::async_trait;
use indexing::{IndexingError, IndexingResult, StorageClient};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct CodexRest {
    base: String,
    http: reqwest::Client,
}

impl CodexRest {
    pub fn new(url: impl Into<String>) -> Self {
        Self::with_client(url, reqwest::Client::new())
    }

    pub fn with_client(url: impl Into<String>, http: reqwest::Client) -> Self {
        Self {
            base: url.into().trim_end_matches('/').to_string(),
            http,
        }
    }

    fn endpoint(&self, p: &str) -> String {
        format!("{}{}", self.base, p)
    }

    async fn post_bytes(&self, filename: &str, bytes: Vec<u8>) -> IndexingResult<String> {
        let r = self
            .http
            .post(self.endpoint("/data"))
            .header("Content-Type", "application/octet-stream")
            .header(
                "Content-Disposition",
                format!(r#"attachment; filename="{}""#, sanitize_filename(filename)),
            )
            .body(bytes)
            .send()
            .await
            .map_err(|e| IndexingError::Transport(e.to_string()))?;
        if !r.status().is_success() {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
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
        // Codex doesn't ship a /health endpoint; HEAD /data is the
        // cheapest probe that doesn't require a known CID.
        match self.http.head(self.endpoint("/data")).send().await {
            Ok(r) => r.status().is_success() || r.status().as_u16() == 405,
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

    #[test]
    fn endpoint_normalises_trailing_slash() {
        let c = CodexRest::new("http://localhost:8080/");
        assert_eq!(c.endpoint("/data"), "http://localhost:8080/data");
    }

    #[test]
    fn sanitize_filename_drops_quotes_and_newlines() {
        assert_eq!(sanitize_filename(r#"a"b\c"#), "a_b_c");
        assert_eq!(sanitize_filename("safe.pdf"), "safe.pdf");
        assert_eq!(sanitize_filename("multi\nline"), "multi_line");
    }
}
