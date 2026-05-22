//! Agnostic document-indexing module for the Logos stack.
//!
//! Three traits — [`StorageClient`], [`DeliveryClient`], [`RegistryClient`] —
//! plus the canonical [`Envelope`] schema. Any Logos application that
//! needs the upload → broadcast → anchor pipeline can depend on this
//! crate alone; the public API exposes only generic primitives and
//! never references any consuming app's types or namespace.
//!
//! See `docs/decisions/002-envelope-schema.md` for the wire format.

#![warn(missing_docs)]

pub mod clients;
pub mod envelope;
pub mod retry;

pub use clients::{DeliveryClient, IndexingError, IndexingResult, RegistryClient, StorageClient};
pub use envelope::{canonical_metadata_hash, Envelope, EnvelopeError};
pub use retry::{with_retry, RetryConfig};
