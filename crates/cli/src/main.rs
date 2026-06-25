use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "phpm", version, about = "PHP dependency manager with a shared global store")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install from composer.lock. Resolves first only if the lock is missing; a STALE
    /// lock (composer.json changed) is NOT re-resolved — run `phpm update` for that.
    Install,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("phpm: error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), cli::install::CliError> {
    let project_dir = std::env::current_dir().map_err(cli::install::CliError::Io)?;
    let store = store::Store::new(cli::store_dir());
    let fetcher = acquire::HttpFetcher::new()?;
    let runner = composer_bridge::SystemRunner;
    let opts = cli::install::InstallOpts { registry_base: cli::registry_base() };
    match cli.command {
        Commands::Install => {
            if !project_dir.join("composer.lock").exists() {
                composer_bridge::update(&runner, &project_dir)?;
            }
            cli::install::install(&project_dir, &store, &fetcher, &runner, &opts)
        }
    }
}
