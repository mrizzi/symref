use symref::deref;
use symref::store;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "symref", about = "Symbolic variable storage and dereferencing")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ingest validated JSON, assign symbolic $VAR references, and store in vars.json
    Store {
        #[arg(long)]
        session: PathBuf,
        #[arg(long)]
        prefix: String,
        #[arg(long)]
        input: Option<PathBuf>,
    },

    /// Substitute $VAR references in text or JSON with stored values
    Deref {
        #[arg(long)]
        session: PathBuf,
        #[arg(long)]
        input: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Store {
            session,
            prefix,
            input,
        } => store::run(&session, &prefix, input.as_deref()),
        Commands::Deref { session, input } => deref::run(&session, input.as_deref()),
    }
}
