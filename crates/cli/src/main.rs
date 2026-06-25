use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "phpm", version, about = "PHP dependency manager with a shared global store")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Materialize vendor/ from composer.lock (resolving first if the lock is missing)
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
