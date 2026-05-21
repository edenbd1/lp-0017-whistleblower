//! Clap CLI definition.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Permissionless anchor CLI for the LP-0017 registry.
#[derive(Debug, Parser)]
#[command(name = "batch-anchor", version, about, long_about = None)]
pub struct Cli {
    /// Path to a TOML config file. Defaults to `./batch-anchor.toml`,
    /// falling back to baked-in defaults if the file is missing.
    #[arg(short, long, default_value = "batch-anchor.toml", global = true)]
    pub config: PathBuf,

    /// Verbose tracing (sets `RUST_LOG=info` if unset).
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Subscribe to the configured Logos Delivery topics and batch-anchor
    /// incoming CIDs to the on-chain registry. Runs until interrupted.
    Watch(WatchArgs),

    /// One-time setup: claim the registry PDA on-chain. Idempotent on
    /// success; safe to re-run.
    Init,

    /// Look up a single CID on-chain. Prints the record as JSON if
    /// anchored, exits 1 if not.
    Lookup(LookupArgs),

    /// List all CIDs currently anchored on-chain. Output: one CID per
    /// line, sorted.
    List,

    /// Publish a local file: upload to Logos Storage, then broadcast
    /// the resulting envelope to the configured Delivery topic. Useful
    /// for manual testing and the demo flow.
    Publish(PublishArgs),

    /// Health probe: report whether storage, delivery, and the
    /// registry program are reachable. Exits 0 if all three are up.
    Doctor,
}

#[derive(Debug, clap::Args)]
pub struct WatchArgs {
    /// Stop after the first batch flush — useful for demos and CI.
    /// Default: run indefinitely.
    #[arg(long)]
    pub once: bool,

    /// Override the per-tick poll interval (seconds).
    #[arg(long)]
    pub poll_interval_secs: Option<u64>,
}

#[derive(Debug, clap::Args)]
pub struct LookupArgs {
    /// The CID to look up. Multiformat string.
    pub cid: String,
}

#[derive(Debug, clap::Args)]
pub struct PublishArgs {
    /// Path to the file to upload.
    pub file: PathBuf,

    /// Optional title for the envelope.
    #[arg(long)]
    pub title: Option<String>,

    /// Optional description.
    #[arg(long)]
    pub description: Option<String>,

    /// Optional tags (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,

    /// Skip the Delivery broadcast (upload only). Default: false.
    #[arg(long)]
    pub no_broadcast: bool,
}
