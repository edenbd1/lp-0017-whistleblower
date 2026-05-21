//! Shared on-chain types for the LP-0017 Whistleblower registry.
//!
//! The same `Registry` / `CidRecord` definitions are decoded by the SPEL
//! guest (`methods/guest/`), the Qt-bridge FFI (`crates/ffi/`), and the
//! off-chain batch CLI (`crates/batch-anchor/`). Keeping the wire format
//! in one crate eliminates drift.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap as Map, string::String, vec::Vec};
#[cfg(feature = "std")]
use std::collections::BTreeMap as Map;

use borsh::{BorshDeserialize, BorshSerialize};

/// Anchor batch size cap. Anything larger than this is rejected with
/// [`RegistryError::BatchTooBig`].
pub const MAX_BATCH: usize = 50;

/// Hard cap on total entries; chosen below the 100 KiB SPEL account-data
/// limit. See `docs/decisions/001-registry-layout.md` for the trade.
pub const MAX_ENTRIES: usize = 900;

/// Stable error codes — numeric values stay frozen across versions.
/// SPEL will surface these as `6000 + code` on the wire.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    /// `metadata_hash` is not exactly 32 bytes.
    InvalidHash = 1,
    /// `timestamp == 0` (or otherwise rejected as bogus).
    BadTimestamp = 2,
    /// The submitted batch contains zero CIDs.
    BatchEmpty = 3,
    /// More than [`MAX_BATCH`] CIDs submitted in one call.
    BatchTooBig = 4,
    /// Registry full; the next insert would exceed [`MAX_ENTRIES`].
    RegistryFull = 5,
    /// `cids`, `metadata_hashes`, `timestamps` were not the same length.
    ArityMismatch = 6,
}

impl RegistryError {
    pub fn code(self) -> u32 {
        self as u32
    }
}

/// One CID's worth of audit data anchored on-chain.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct CidRecord {
    /// `sha256(canonical_metadata_json)` — see ADR-002.
    pub metadata_hash: [u8; 32],
    /// Unix seconds (epoch) — i64 to match Solana / SPEL convention.
    pub anchor_timestamp: i64,
    /// Account that submitted this anchor. Audit-trail only.
    pub anchored_by: [u8; 32],
    /// Wire-format version. Always `1` at v1.
    pub version: u8,
}

impl CidRecord {
    pub fn new(metadata_hash: [u8; 32], ts: i64, anchored_by: [u8; 32]) -> Self {
        Self {
            metadata_hash,
            anchor_timestamp: ts,
            anchored_by,
            version: 1,
        }
    }
}

/// Top-level account state. One PDA at seed `b"registry"` holds the
/// entire registry; Borsh sorts BTreeMap by key so the encoding is
/// deterministic across SPEL guest executions.
#[derive(Clone, Debug, Default, BorshSerialize, BorshDeserialize)]
pub struct Registry {
    /// CID -> CidRecord.
    ///
    /// Using `BTreeMap` (not `HashMap`) keeps Borsh's byte output
    /// deterministic. SPEL's chained-call validation relies on this.
    pub entries: Map<String, CidRecord>,
}

impl Registry {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, cid: &str) -> bool {
        self.entries.contains_key(cid)
    }

    /// Insert if absent. Returns `true` if the CID was newly anchored.
    /// Returns `false` if the CID was already present (idempotent skip).
    pub fn try_insert(&mut self, cid: String, record: CidRecord) -> Result<bool, RegistryError> {
        if self.entries.len() >= MAX_ENTRIES && !self.entries.contains_key(&cid) {
            return Err(RegistryError::RegistryFull);
        }
        if self.entries.contains_key(&cid) {
            return Ok(false);
        }
        self.entries.insert(cid, record);
        Ok(true)
    }
}

/// Validate the three parallel-vector inputs of `index_batch` before
/// touching state. Pure function, used by both guest and tests.
pub fn validate_batch(
    cids: &[String],
    hashes: &[[u8; 32]],
    timestamps: &[u32],
) -> Result<(), RegistryError> {
    if cids.is_empty() {
        return Err(RegistryError::BatchEmpty);
    }
    if cids.len() > MAX_BATCH {
        return Err(RegistryError::BatchTooBig);
    }
    if cids.len() != hashes.len() || cids.len() != timestamps.len() {
        return Err(RegistryError::ArityMismatch);
    }
    for ts in timestamps {
        if *ts == 0 {
            return Err(RegistryError::BadTimestamp);
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    fn cid(n: u8) -> String {
        format!("zDvZRwzk{:056}", n)
    }
    fn hash(n: u8) -> [u8; 32] {
        [n; 32]
    }
    fn signer() -> [u8; 32] {
        [0xAA; 32]
    }

    #[test]
    fn empty_registry_is_empty() {
        let r = Registry::default();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn insert_increments_len() {
        let mut r = Registry::default();
        assert!(r.try_insert(cid(1), CidRecord::new(hash(1), 100, signer())).unwrap());
        assert_eq!(r.len(), 1);
        assert!(r.contains(&cid(1)));
    }

    #[test]
    fn duplicate_insert_is_silent_skip() {
        let mut r = Registry::default();
        r.try_insert(cid(1), CidRecord::new(hash(1), 100, signer())).unwrap();
        let added = r.try_insert(cid(1), CidRecord::new(hash(99), 999, signer())).unwrap();
        assert!(!added, "duplicate must report not-newly-inserted");
        assert_eq!(r.len(), 1);
        // First write wins:
        assert_eq!(r.entries.get(&cid(1)).unwrap().metadata_hash, hash(1));
        assert_eq!(r.entries.get(&cid(1)).unwrap().anchor_timestamp, 100);
    }

    #[test]
    fn registry_full_rejects_new_cids_only() {
        let mut r = Registry::default();
        for i in 0..MAX_ENTRIES {
            r.try_insert(format!("cid:{i}"), CidRecord::new(hash(0), 1, signer())).unwrap();
        }
        // Existing CID still returns Ok(false).
        assert_eq!(
            r.try_insert("cid:0".into(), CidRecord::new(hash(0), 1, signer())).unwrap(),
            false
        );
        // Brand new CID rejected with RegistryFull.
        assert_eq!(
            r.try_insert("cid:new".into(), CidRecord::new(hash(0), 1, signer())).unwrap_err(),
            RegistryError::RegistryFull
        );
    }

    #[test]
    fn validate_batch_happy_path() {
        let cids = vec![cid(1), cid(2)];
        let hashes = vec![hash(1), hash(2)];
        let ts = vec![100u32, 200u32];
        validate_batch(&cids, &hashes, &ts).unwrap();
    }

    #[test]
    fn validate_batch_rejects_empty() {
        assert_eq!(
            validate_batch(&[], &[], &[]).unwrap_err(),
            RegistryError::BatchEmpty
        );
    }

    #[test]
    fn validate_batch_rejects_oversized() {
        let cids: Vec<String> = (0..=MAX_BATCH as u8).map(cid).collect();
        let hashes: Vec<[u8; 32]> = (0..=MAX_BATCH as u8).map(hash).collect();
        let ts: Vec<u32> = (0..=MAX_BATCH as u32).map(|i| i + 1).collect();
        assert_eq!(
            validate_batch(&cids, &hashes, &ts).unwrap_err(),
            RegistryError::BatchTooBig
        );
    }

    #[test]
    fn validate_batch_rejects_arity_mismatch() {
        assert_eq!(
            validate_batch(&[cid(1), cid(2)], &[hash(1)], &[100u32, 200u32]).unwrap_err(),
            RegistryError::ArityMismatch
        );
    }

    #[test]
    fn validate_batch_rejects_zero_timestamp() {
        assert_eq!(
            validate_batch(&[cid(1)], &[hash(1)], &[0u32]).unwrap_err(),
            RegistryError::BadTimestamp
        );
    }

    #[test]
    fn borsh_roundtrip_is_deterministic() {
        let mut r = Registry::default();
        for i in 0..10 {
            r.try_insert(cid(i), CidRecord::new(hash(i), 100 + i as i64, signer())).unwrap();
        }
        // Re-serialize after inserting in a different order — Borsh +
        // BTreeMap means the bytes must be identical.
        let bytes_a = borsh::to_vec(&r).unwrap();
        let mut r2 = Registry::default();
        for i in (0..10).rev() {
            r2.try_insert(cid(i), CidRecord::new(hash(i), 100 + i as i64, signer())).unwrap();
        }
        let bytes_b = borsh::to_vec(&r2).unwrap();
        assert_eq!(bytes_a, bytes_b, "borsh encoding must be order-independent");
    }

    #[test]
    fn error_codes_match_documented_values() {
        // Frozen — see docs/decisions/001-registry-layout.md §"Error codes".
        assert_eq!(RegistryError::InvalidHash.code(), 1);
        assert_eq!(RegistryError::BadTimestamp.code(), 2);
        assert_eq!(RegistryError::BatchEmpty.code(), 3);
        assert_eq!(RegistryError::BatchTooBig.code(), 4);
        assert_eq!(RegistryError::RegistryFull.code(), 5);
        assert_eq!(RegistryError::ArityMismatch.code(), 6);
    }
}
