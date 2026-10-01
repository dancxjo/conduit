//! Operator entrance for finite self-hosted Line relay infrastructure.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[cfg(not(test))]
#[path = "../rendezvous_relay.rs"]
mod rendezvous_relay;

#[derive(Debug, Parser)]
#[command(
    name = "conduit-relay",
    about = "Operate one finite Conduit Line relay"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create one private relay slot and two exact endpoint descriptors.
    Provision {
        #[arg(long)]
        relay_address: String,
        #[arg(long)]
        relay_url: String,
        #[arg(long)]
        server_identity: String,
        #[arg(long)]
        certificate_sha256: String,
        #[arg(long)]
        first_host_id: String,
        #[arg(long)]
        first_boot_id: String,
        #[arg(long)]
        second_host_id: String,
        #[arg(long)]
        second_boot_id: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u64).range(1..=3600))]
        expires_in_seconds: u64,
        #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=3))]
        maximum_attempts: u8,
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_provision: bool,
    },
    /// Serve one configured two-endpoint opaque relay slot over pinned WSS.
    Serve {
        #[arg(long)]
        bind: String,
        #[arg(long)]
        public_url: String,
        #[arg(long)]
        tls_cert: PathBuf,
        #[arg(long)]
        tls_key: PathBuf,
        #[arg(long)]
        slot: PathBuf,
        #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u64).range(1..=3600))]
        accept_timeout_seconds: u64,
        #[arg(long, required = true, action = clap::ArgAction::SetTrue)]
        authorize_network: bool,
    },
}

#[cfg(not(test))]
fn main() {
    let result = match Cli::parse().command {
        Command::Provision {
            relay_address,
            relay_url,
            server_identity,
            certificate_sha256,
            first_host_id,
            first_boot_id,
            second_host_id,
            second_boot_id,
            output,
            expires_in_seconds,
            maximum_attempts,
            authorize_provision,
        } => rendezvous_relay::provision(rendezvous_relay::ProvisionOptions {
            relay_address,
            relay_url,
            server_identity,
            certificate_sha256,
            first_host_id,
            first_boot_id,
            second_host_id,
            second_boot_id,
            output,
            expires_in_seconds,
            maximum_attempts,
            authorize_provision,
        }),
        Command::Serve {
            bind,
            public_url,
            tls_cert,
            tls_key,
            slot,
            accept_timeout_seconds,
            authorize_network,
        } => rendezvous_relay::serve(rendezvous_relay::ServeOptions {
            bind,
            public_url,
            tls_cert,
            tls_key,
            slot,
            accept_timeout_seconds,
            authorize_network,
        }),
    };
    if let Err(error) = result {
        eprintln!("conduit-relay error: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
fn main() {}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn operator_help_names_relay_without_expanding_the_product_cli() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("conduit-relay"));
        assert!(help.contains("provision"));
        assert!(help.contains("serve"));
    }
}
