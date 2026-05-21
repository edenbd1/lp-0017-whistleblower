//! Trait surface for storage / delivery / registry backends.
//!
//! Each trait represents *one* responsibility and intentionally hides
//! the underlying transport (Codex REST, nwaku REST, LEZ sequencer
//! RPC). A consuming app picks any implementation — or substitutes a
//! mock — without touching the indexing logic itself.

use async_trait::async_trait;
use registry_core::CidRecord;
use thiserror::Error;

use crate::envelope::Envelope;

/// Module-level error covering every backend.
#[derive(Debug, Error)]
pub enum IndexingError {
    /// A wrapped transport / IO failure.
    #[error("transport: {0}")]
    Transport(String),

    /// Backend rejected the call for a reason it could articulate.
    #[error("backend: {0}")]
    Backend(String),

    /// Envelope or payload didn't validate.
    #[error("envelope: {0}")]
    Envelope(#[from] crate::envelope::EnvelopeError),

    /// Registry-side error surfaced from the on-chain program.
    #[error("registry error code {0}")]
    RegistryCode(u32),

    /// Catch-all for unexpected backend states.
    #[error("unexpected: {0}")]
    Unexpected(String),
}

/// Convenience alias.
pub type IndexingResult<T> = Result<T, IndexingError>;

/// Upload bytes (or a local file) to a content-addressed store and get
/// back the CID. Implementations are expected to handle retry / chunking
/// internally — the trait surface stays small.
#[async_trait]
pub trait StorageClient: Send + Sync {
    /// Health probe. `true` means a subsequent `upload_*` will reach
    /// the backend (transport-level only — does not exercise capacity).
    async fn healthy(&self) -> bool;

    /// Upload a local file. Returns the CID as printed by the backend
    /// (multiformat string for Codex; opaque for tests).
    async fn upload_file(&self, path: &std::path::Path) -> IndexingResult<String>;

    /// Upload an in-memory byte slice; `filename` is advisory for
    /// `Content-Disposition`.
    async fn upload_bytes(&self, filename: &str, bytes: &[u8]) -> IndexingResult<String>;
}

/// Publish to and drain from a Logos Delivery (nwaku) topic.
///
/// `drain` is destructive on the nwaku relay-REST side: consecutive
/// calls return only new messages since the last GET. Use [`Self::query_store`]
/// for catch-up after an outage (24 h lookback recommended).
#[async_trait]
pub trait DeliveryClient: Send + Sync {
    /// Health probe.
    async fn healthy(&self) -> bool;

    /// Subscribe (idempotent). Required before `drain` / `query_store`.
    async fn subscribe(&self, topic: &str) -> IndexingResult<()>;

    /// Publish a JSON envelope to the topic.
    async fn publish(&self, topic: &str, env: &Envelope) -> IndexingResult<()>;

    /// Drain the relay queue. Returns parsed envelopes that pass
    /// validation; malformed payloads are silently dropped.
    async fn drain(&self, topic: &str) -> IndexingResult<Vec<Envelope>>;

    /// Catch-up via the nwaku store-protocol. `start_ns` is a nanosecond
    /// Unix timestamp — `0` to fetch everything the store remembers.
    async fn query_store(&self, topic: &str, start_ns: u128) -> IndexingResult<Vec<Envelope>>;
}

/// Read / write the on-chain CID registry.
///
/// Implementations are split per app: tests use an in-memory mock; the
/// CLI shells out to `lgs` + the SPEL CLI; the Basecamp module calls
/// the cdylib in `crates/ffi`.
#[async_trait]
pub trait RegistryClient: Send + Sync {
    /// Returns `true` if the program PDA exists and the account has been
    /// initialised. Used at startup before issuing `index_batch`.
    async fn is_initialised(&self) -> IndexingResult<bool>;

    /// One-time setup: claim the registry PDA. Idempotent on success.
    async fn init(&self) -> IndexingResult<String>;

    /// Submit a batch of CIDs. Returns the tx hash on success.
    async fn index_batch(
        &self,
        cids: &[String],
        metadata_hashes: &[[u8; 32]],
        timestamps: &[u32],
    ) -> IndexingResult<String>;

    /// Read a single record from the registry. `None` = not anchored.
    async fn lookup(&self, cid: &str) -> IndexingResult<Option<CidRecord>>;

    /// Snapshot all anchored CIDs. Used at startup to seed the off-chain
    /// dedup set — see ADR-001.
    async fn anchored_cid_set(&self) -> IndexingResult<std::collections::HashSet<String>>;
}
