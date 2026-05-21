//! `batch-anchor publish <file>` — upload + broadcast a single file.
//!
//! Manual testing aid. Uploads to Logos Storage, builds the canonical
//! envelope, and (unless `--no-broadcast`) publishes to the first
//! configured Delivery topic.

use crate::cli::PublishArgs;
use crate::config::Config;
use crate::delivery::NwakuRest;
use crate::storage::CodexRest;
use indexing::{canonical_metadata_hash, DeliveryClient, Envelope, StorageClient};
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn run(cfg: &Config, args: &PublishArgs) -> anyhow::Result<()> {
    let storage = CodexRest::new(cfg.storage.url.clone());
    let bytes = tokio::fs::read(&args.file).await?;
    let size_bytes = bytes.len() as u64;
    let filename = args
        .file
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let cid = storage.upload_bytes(&filename, &bytes).await?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let content_type = mime_from_extension(&args.file);
    let metadata_hash = canonical_metadata_hash(
        args.title.as_deref().or(Some(&filename)),
        args.description.as_deref(),
        content_type.as_deref(),
        Some(size_bytes),
        &args.tags,
    );

    let env = Envelope {
        v: 1,
        cid: cid.clone(),
        metadata_hash,
        timestamp,
        title: args.title.clone().or_else(|| Some(filename.clone())),
        description: args.description.clone(),
        content_type,
        size_bytes: Some(size_bytes),
        tags: args.tags.clone(),
    };

    println!("uploaded:");
    println!("  cid = {cid}");
    println!("  size_bytes = {size_bytes}");
    println!("  metadata_hash = {}", env.metadata_hash);

    if !args.no_broadcast {
        let delivery = NwakuRest::new(cfg.delivery.url.clone());
        let topic = cfg
            .delivery
            .topics
            .first()
            .map(String::as_str)
            .unwrap_or("/whistleblower/1/document-broadcast/json");
        delivery.subscribe(topic).await.ok();
        delivery.publish(topic, &env).await?;
        println!("broadcast to: {topic}");
    }
    Ok(())
}

fn mime_from_extension(path: &std::path::Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    Some(
        match ext.as_str() {
            "pdf" => "application/pdf",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "txt" => "text/plain",
            "md" => "text/markdown",
            "json" => "application/json",
            "html" => "text/html",
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            _ => "application/octet-stream",
        }
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn known_extensions_map_to_mime() {
        assert_eq!(mime_from_extension(Path::new("a.pdf")).unwrap(), "application/pdf");
        assert_eq!(mime_from_extension(Path::new("img.PNG")).unwrap(), "image/png");
        assert_eq!(mime_from_extension(Path::new("v.mp4")).unwrap(), "video/mp4");
    }

    #[test]
    fn unknown_extension_falls_back_to_octet_stream() {
        assert_eq!(
            mime_from_extension(Path::new("foo.bin")).unwrap(),
            "application/octet-stream"
        );
    }

    #[test]
    fn no_extension_returns_none() {
        assert_eq!(mime_from_extension(Path::new("noext")), None);
    }
}
