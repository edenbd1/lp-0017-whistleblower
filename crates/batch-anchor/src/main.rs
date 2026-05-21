//! batch-anchor — permissionless anchor CLI for the LP-0017 registry.
//!
//! Subscribes to a Logos Delivery topic via nwaku REST, deduplicates
//! incoming envelopes against the on-chain registry, and submits
//! accumulated CIDs in batches up to `MAX_BATCH`.
//!
//! Run `batch-anchor --help` for the available subcommands.

fn main() {
    eprintln!("batch-anchor: scaffold — subcommands land in later commits.");
}
