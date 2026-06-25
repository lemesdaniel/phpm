use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "phpm",
    version,
    about = "PHP dependency manager with a shared global store"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install from composer.lock. Resolves first only if the lock is missing; a STALE
    /// lock (composer.json changed) is NOT re-resolved — run `phpm update` for that.
    Install {
        #[arg(long)]
        no_dev: bool,
    },
    /// Add one or more dependencies (delegates resolution to Composer), then install
    Require {
        packages: Vec<String>,
        #[arg(long)]
        no_dev: bool,
    },
    /// Remove one or more dependencies, then install
    Remove {
        packages: Vec<String>,
        #[arg(long)]
        no_dev: bool,
    },
    /// Re-resolve and update composer.lock, then install
    Update {
        #[arg(long)]
        no_dev: bool,
    },
    /// Remove unreferenced packages from the global store (dry run unless --prune)
    Gc {
        #[arg(long)]
        prune: bool,
    },
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

    match cli.command {
        Commands::Install { no_dev } => {
            let opts = cli::install::InstallOpts {
                registry_base: cli::registry_base(),
                no_dev,
            };
            if !project_dir.join("composer.lock").exists() {
                composer_bridge::update(&runner, &project_dir)?;
            }
            cli::install::install(&project_dir, &store, &fetcher, &runner, &opts)?;
        }
        Commands::Require { packages, no_dev } => {
            let opts = cli::install::InstallOpts {
                registry_base: cli::registry_base(),
                no_dev,
            };
            composer_bridge::require(&runner, &project_dir, &packages)?;
            cli::install::install(&project_dir, &store, &fetcher, &runner, &opts)?;
        }
        Commands::Remove { packages, no_dev } => {
            let opts = cli::install::InstallOpts {
                registry_base: cli::registry_base(),
                no_dev,
            };
            composer_bridge::remove(&runner, &project_dir, &packages)?;
            cli::install::install(&project_dir, &store, &fetcher, &runner, &opts)?;
        }
        Commands::Update { no_dev } => {
            let opts = cli::install::InstallOpts {
                registry_base: cli::registry_base(),
                no_dev,
            };
            composer_bridge::update(&runner, &project_dir)?;
            cli::install::install(&project_dir, &store, &fetcher, &runner, &opts)?;
        }
        Commands::Gc { prune } => {
            let report = cli::gc_run(&store, &cli::registry_base(), prune)?;
            if prune {
                println!("phpm: gc removed {} package(s)", report.removed);
            } else {
                println!(
                    "phpm: gc would remove {} package(s) (run with --prune to delete)",
                    report.would_remove
                );
            }
        }
    }
    Ok(())
}
