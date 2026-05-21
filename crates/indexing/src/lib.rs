//! Agnostic document-indexing module for the Logos stack.
//!
//! This crate ships three traits — `StorageClient`, `DeliveryClient`,
//! `RegistryClient` — plus the canonical envelope schema. Any Logos
//! application that needs the upload → broadcast → anchor pipeline can
//! depend on this crate alone; no Whistleblower-specific types leak.
//!
//! See `docs/decisions/002-envelope-schema.md` for the wire format.

#![warn(missing_docs)]
