//! `batch-anchor` — permissionless anchor CLI for the LP-0017 registry.

use anyhow::Context;
use batch_anchor::{cli, cmd, config::Config};
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    init_tracing(args.verbose);
    let cfg = Config::load(Some(&args.config))
        .with_context(|| format!("load config: {}", args.config.display()))?;
    match &args.command {
        cli::Command::Watch(a) => cmd::watch::run(&cfg, a).await,
        cli::Command::Init => cmd::init::run(&cfg).await,
        cli::Command::Lookup(a) => cmd::lookup::run(&cfg, a).await,
        cli::Command::List => cmd::list::run(&cfg).await,
        cli::Command::Publish(a) => cmd::publish::run(&cfg, a).await,
        cli::Command::Doctor => cmd::doctor::run(&cfg).await,
    }
}

fn init_tracing(verbose: bool) {
    let default = if verbose { "debug,hyper=info,reqwest=info" } else { "info,hyper=warn,reqwest=warn" };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
