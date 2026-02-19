use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;
mod config;
mod mdk_helper;
mod nostr_client;
mod output;

#[derive(Parser)]
#[command(name = "mdk")]
#[command(about = "CLI for MLS-encrypted messaging over Nostr")]
#[command(version)]
struct Cli {
    /// Path to nsec key file (or set MDK_KEY_FILE env var)
    #[arg(long, env = "MDK_KEY_FILE")]
    key_file: Option<String>,

    /// Path to SQLite database (default: ~/.mdk/state.db)
    #[arg(long, env = "MDK_DB_PATH")]
    db_path: Option<String>,

    /// Relay URLs (comma-separated, or set MDK_RELAYS env var)
    #[arg(long, env = "MDK_RELAYS", value_delimiter = ',')]
    relays: Option<Vec<String>>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize MDK with a new or existing identity
    Init {
        /// Path to nsec key file (hex format)
        #[arg(long)]
        nsec_file: Option<String>,
    },

    /// Publish MLS key package to relays (kind 443)
    PublishKeyPackage,

    /// List pending welcome invitations
    ListWelcomes,

    /// Accept a welcome invitation and join the group
    AcceptWelcome {
        /// Event ID of the welcome to accept
        event_id: String,
    },

    /// List active groups
    ListGroups,

    /// Send a message to a group
    Send {
        /// Group ID (hex)
        group_id: String,
        /// Message content
        message: String,
    },

    /// Receive and display new messages (polls relays once)
    Receive {
        /// Group ID to receive from (optional, receives from all if omitted)
        #[arg(long)]
        group_id: Option<String>,
        /// Fetch only events after this timestamp or event ID
        #[arg(long)]
        since: Option<String>,
        /// Stream new messages continuously (NDJSON output)
        #[arg(long)]
        watch: bool,
        /// Poll interval in seconds (used with --watch)
        #[arg(long, default_value = "5")]
        poll_interval: u64,
    },

    /// Show identity info (npub, pubkey)
    Whoami,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Load config
    let config = config::Config::load(&cli)?;

    // Dispatch command
    match cli.command {
        Commands::Init { nsec_file } => commands::init::run(&config, nsec_file).await,
        Commands::PublishKeyPackage => commands::publish_key_package::run(&config).await,
        Commands::ListWelcomes => commands::list_welcomes::run(&config).await,
        Commands::AcceptWelcome { event_id } => {
            commands::accept_welcome::run(&config, &event_id).await
        }
        Commands::ListGroups => commands::list_groups::run(&config).await,
        Commands::Send { group_id, message } => {
            commands::send::run(&config, &group_id, &message).await
        }
        Commands::Receive { group_id, since, watch, poll_interval } => {
            commands::receive::run(&config, group_id.as_deref(), since.as_deref(), watch, poll_interval).await
        }
        Commands::Whoami => commands::whoami::run(&config).await,
    }
}
