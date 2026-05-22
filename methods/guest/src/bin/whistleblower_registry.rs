//! SPEL guest for the LP-0017 Whistleblower CID registry.
//!
//! Two instructions:
//!
//! * `init_registry` — claim the registry PDA (idempotent at the SPEL
//!   layer; double-init returns `AccountAlreadyInitialized`).
//! * `index_batch` — append up to [`MAX_BATCH`] `(cid, metadata_hash,
//!   anchor_timestamp)` triples. In-program dedup via
//!   `Registry::try_insert`; duplicates are silently skipped, matching
//!   the spec's idempotency requirement.
//!
//! Wire format follows ADR-001 — three parallel `Vec` arguments,
//! single PDA at `seed = literal("registry")`, Borsh-encoded
//! `Registry { entries: BTreeMap<String, CidRecord> }` in the account
//! data.

#![no_main]

use spel_framework::prelude::*;
use nssa_core::account::Data;

use registry_core::{CidRecord, Registry, MAX_BATCH};

risc0_zkvm::guest::entry!(main);

// Error codes are mirrored from registry_core::RegistryError so the
// guest doesn't need to depend on the std-only error enum.
const E_INVALID_HASH:   u32 = 1;
const E_BAD_TIMESTAMP:  u32 = 2;
const E_BATCH_EMPTY:    u32 = 3;
const E_BATCH_TOO_BIG:  u32 = 4;
const E_REGISTRY_FULL:  u32 = 5;
const E_ARITY_MISMATCH: u32 = 6;

#[lez_program]
mod whistleblower_registry {
    #[allow(unused_imports)]
    use super::*;

    /// Claim the registry PDA. The program owns it; the signer is the
    /// entity paying for the account-init transaction (anyone — the
    /// registry is permissionless). Double-init returns
    /// `AccountAlreadyInitialized` (SpelError code 1002), which the
    /// off-chain caller treats as a no-op success.
    #[instruction]
    pub fn init_registry(
        #[account(init, pda = [literal("registry")])]
        mut registry: AccountWithMetadata,
        #[account(signer)]
        payer: AccountWithMetadata,
    ) -> SpelResult {
        let empty = Registry::default();
        let bytes = borsh::to_vec(&empty).map_err(|e| SpelError::SerializationError {
            message: e.to_string(),
        })?;
        registry.account.data = Data::try_from(bytes).map_err(|_| {
            SpelError::custom(E_REGISTRY_FULL, "initial registry exceeds DATA_MAX_LENGTH".to_string())
        })?;
        Ok(SpelOutput::execute(vec![registry, payer], vec![]))
    }

    /// Anchor a batch of CIDs.
    ///
    /// Three parallel vectors of equal length keep the SPEL IDL
    /// codegen happy (tuples and structs do not survive the IDL JSON
    /// schema). Duplicates are silently skipped — idempotency is
    /// enforced inside the program, matching the spec's "re-anchoring
    /// an already-registered CID does not fail" requirement.
    ///
    /// `anchor_timestamps` are u32 unix-seconds on the wire (the spel
    /// CLI can serialize u32 natively but not i64); stored as i64.
    #[instruction]
    pub fn index_batch(
        #[account(mut, pda = [literal("registry")])]
        mut registry: AccountWithMetadata,
        #[account(signer)]
        anchorer: AccountWithMetadata,
        cids: Vec<String>,
        metadata_hashes: Vec<[u8; 32]>,
        anchor_timestamps: Vec<u32>,
    ) -> SpelResult {
        // 1. Validate the batch shape before touching state.
        let n = cids.len();
        if n == 0 {
            return Err(SpelError::custom(E_BATCH_EMPTY, "batch is empty".to_string()));
        }
        if n > MAX_BATCH {
            return Err(SpelError::custom(
                E_BATCH_TOO_BIG,
                format!("batch size {} > MAX_BATCH {}", n, MAX_BATCH),
            ));
        }
        if metadata_hashes.len() != n || anchor_timestamps.len() != n {
            return Err(SpelError::custom(
                E_ARITY_MISMATCH,
                format!(
                    "cids={}, metadata_hashes={}, anchor_timestamps={} (must match)",
                    n,
                    metadata_hashes.len(),
                    anchor_timestamps.len()
                ),
            ));
        }
        for ts in &anchor_timestamps {
            if *ts == 0 {
                return Err(SpelError::custom(
                    E_BAD_TIMESTAMP,
                    "timestamp must be non-zero".to_string(),
                ));
            }
        }

        // 2. Decode the current registry state.
        let mut state: Registry =
            borsh::from_slice(registry.account.data.as_ref()).map_err(|e| {
                SpelError::SerializationError {
                    message: format!("registry decode: {e}"),
                }
            })?;

        // 3. Insert each new CID, silently skipping duplicates.
        let signer_key = *anchorer.account_id.value();
        for ((cid, hash), ts) in cids
            .into_iter()
            .zip(metadata_hashes.into_iter())
            .zip(anchor_timestamps.into_iter())
        {
            // contains_key short-circuit — idempotent re-anchor.
            if state.contains(&cid) {
                continue;
            }
            // Capacity gate.
            if state.len() >= registry_core::MAX_ENTRIES {
                return Err(SpelError::custom(
                    E_REGISTRY_FULL,
                    format!(
                        "registry full (>= {} entries) — open a new program version",
                        registry_core::MAX_ENTRIES
                    ),
                ));
            }
            state
                .entries
                .insert(cid, CidRecord::new(hash, ts as i64, signer_key));
        }

        // 4. Re-encode + write.
        let new_bytes = borsh::to_vec(&state).map_err(|e| SpelError::SerializationError {
            message: format!("registry re-encode: {e}"),
        })?;
        registry.account.data = Data::try_from(new_bytes).map_err(|_| {
            SpelError::custom(E_REGISTRY_FULL, "post-write registry exceeds DATA_MAX_LENGTH".to_string())
        })?;

        Ok(SpelOutput::execute(vec![registry, anchorer], vec![]))
    }
}

// Sanity: any future bump of MAX_BATCH past u32::MAX trips a compile
// error here instead of silently truncating on the wire.
const _: () = assert!(MAX_BATCH <= u32::MAX as usize);

// Silence unused_imports warnings in the host build (the imports are
// used inside the #[lez_program] macro expansion).
#[allow(dead_code)]
fn _force_use_of_borsh() {
    let _ = borsh::to_vec::<Registry>(&Registry::default());
    let _: u32 = E_INVALID_HASH;
}
