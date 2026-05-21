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
risc0_zkvm::guest::entry!(main);

use registry_core::{
    validate_batch, CidRecord, Registry, RegistryError, MAX_BATCH,
};

#[lez_program]
mod whistleblower_registry {
    use super::*;

    /// Pulled into the IDL `accounts[]` so `spel inspect` can decode
    /// the registry PDA without bespoke tooling.
    #[account_type]
    pub use registry_core::Registry as RegistryAccount;

    /// Claim the registry PDA. The program is the owner; the signer
    /// is the entity that pays for the account-init transaction
    /// (anyone — the registry is permissionless).
    #[instruction]
    pub fn init_registry(
        ctx: ProgramContext,
        #[account(init, pda = const("registry"))] registry: AccountWithMetadata,
        #[account(signer)] payer: AccountWithMetadata,
    ) -> SpelResult {
        let _ = ctx; // unused — the PDA seed is constant.
        let initial = Registry::default();
        let bytes = borsh::to_vec(&initial)
            .map_err(|e| SpelError::custom(99, &format!("borsh encode: {e}")))?;
        let mut registry = registry;
        registry.account.data = bytes
            .try_into()
            .map_err(|_| SpelError::custom(99, "initial registry larger than DATA_MAX_LENGTH"))?;
        Ok(SpelOutput::execute(vec![registry, payer], vec![]))
    }

    /// Append up to [`MAX_BATCH`] CIDs to the registry. Three parallel
    /// vectors keep the SPEL IDL codegen happy (tuples and structs do
    /// not survive IDL serialization).
    #[instruction]
    pub fn index_batch(
        ctx: ProgramContext,
        #[account(mut, pda = const("registry"), owner = ctx.self_program_id)]
        registry: AccountWithMetadata,
        #[account(signer)] anchorer: AccountWithMetadata,
        cids: Vec<String>,
        metadata_hashes: Vec<[u8; 32]>,
        anchor_timestamps: Vec<u32>,
    ) -> SpelResult {
        validate_batch(&cids, &metadata_hashes, &anchor_timestamps)
            .map_err(map_err)?;

        let mut state: Registry = borsh::from_slice(&registry.account.data)
            .map_err(|e| SpelError::custom(98, &format!("registry decode: {e}")))?;

        let signer_key = *anchorer.account_id.value();
        for ((cid, hash), ts) in cids
            .into_iter()
            .zip(metadata_hashes.into_iter())
            .zip(anchor_timestamps.into_iter())
        {
            let record = CidRecord::new(hash, ts as i64, signer_key);
            // try_insert silently skips duplicates → idempotent batch.
            state.try_insert(cid, record).map_err(map_err)?;
        }

        let bytes = borsh::to_vec(&state)
            .map_err(|e| SpelError::custom(99, &format!("borsh encode: {e}")))?;
        let mut registry = registry;
        registry.account.data = bytes
            .try_into()
            .map_err(|_| SpelError::custom(99, "registry larger than DATA_MAX_LENGTH"))?;
        Ok(SpelOutput::execute(vec![registry, anchorer], vec![]))
    }
}

fn map_err(e: RegistryError) -> SpelError {
    let name: &'static str = match e {
        RegistryError::InvalidHash => "InvalidHash",
        RegistryError::BadTimestamp => "BadTimestamp",
        RegistryError::BatchEmpty => "BatchEmpty",
        RegistryError::BatchTooBig => "BatchTooBig",
        RegistryError::RegistryFull => "RegistryFull",
        RegistryError::ArityMismatch => "ArityMismatch",
    };
    SpelError::custom(e.code(), name)
}

// Static assertion so a future bump of registry-core that lifts
// MAX_BATCH past u32::MAX trips a compile error here instead of
// silently truncating on the wire.
const _: () = assert!(MAX_BATCH <= u32::MAX as usize);
