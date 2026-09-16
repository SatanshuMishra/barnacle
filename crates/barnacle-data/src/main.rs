use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Problem;
use barnacle_catalog::curation::curate;
use barnacle_catalog::curation::validate;
use barnacle_data::report::diff_report;
use barnacle_data::report::problems_report;
use barnacle_data::store::DataDir;
use barnacle_data::store::StoreError;
use barnacle_data::store::build;
use barnacle_data::store::download;
use clap::Parser;
use clap::Subcommand;

#[derive(Parser)]
#[command(
    name = "barnacle-data",
    about = "Download World of Warships game data and build Barnacle's ship catalog"
)]
struct Cli {
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
    #[arg(long, default_value = "curation/ships.toml")]
    curation: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Download the newest published build (or --build N) and build its catalog")]
    Sync {
        #[arg(long)]
        build: Option<u32>,
    },
    #[command(about = "Check curation against a catalog (default: the newest built)")]
    Validate { catalog: Option<String> },
    #[command(
        about = "Compare two catalogs (default: current against newest) and show curation results"
    )]
    Diff {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
    },
    #[command(about = "Make a validated catalog the one the bot loads on its next start")]
    Use { catalog: String },
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("curation file {path} could not be read")]
    CurationIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("curation file {path} is not valid")]
    CurationToml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("the async runtime could not start")]
    Runtime(#[source] std::io::Error),
    #[error("catalog {catalog} has curation problems; run `barnacle-data validate {catalog}`")]
    Invalid { catalog: String },
}

fn load_curation(path: &Path) -> Result<CurationConfig, AppError> {
    let text = std::fs::read_to_string(path).map_err(|source| AppError::CurationIo {
        path: path.to_owned(),
        source,
    })?;
    CurationConfig::from_toml(&text).map_err(|source| AppError::CurationToml {
        path: path.to_owned(),
        source,
    })
}

fn problems_for(data: &DataDir, curation: &Path, name: &str) -> Result<Vec<Problem>, AppError> {
    let config = load_curation(curation)?;
    let catalog = data.load(name)?;
    let curated = curate(&catalog, &config);
    Ok(validate(&catalog, &config, &curated))
}

fn run(cli: &Cli) -> Result<ExitCode, AppError> {
    let data = DataDir::new(&cli.data_dir);
    match &cli.command {
        Command::Sync { build: requested } => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(AppError::Runtime)?;
            let downloaded = runtime.block_on(download(&data, *requested))?;
            let built = build(&data, &downloaded)?;
            let source = if downloaded.refreshed {
                "refreshed game data"
            } else {
                "game data"
            };
            println!(
                "built catalog {} with {} ships from {source} at commit {}; next: barnacle-data diff",
                built.name,
                built.catalog.ships.len(),
                downloaded.data_repo_commit
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Validate { catalog } => {
            let name = match catalog {
                Some(name) => name.clone(),
                None => data.newest()?,
            };
            let problems = problems_for(&data, &cli.curation, &name)?;
            println!("{}", problems_report(&name, &problems));
            Ok(if problems.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Command::Diff { from, to } => {
            let config = load_curation(&cli.curation)?;
            let to = match to {
                Some(name) => name.clone(),
                None => data.newest()?,
            };
            let from = match from {
                Some(name) => Some(name.clone()),
                None => data.current()?,
            };
            let old = from.as_deref().map(|name| data.load(name)).transpose()?;
            let new = data.load(&to)?;
            println!("{}", diff_report(old.as_ref(), &new, &config));
            Ok(ExitCode::SUCCESS)
        }
        Command::Use { catalog } => {
            if !problems_for(&data, &cli.curation, catalog)?.is_empty() {
                return Err(AppError::Invalid {
                    catalog: catalog.clone(),
                });
            }
            data.set_current(catalog)?;
            println!("the bot will load {catalog} on its next start");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            std::iter::successors(std::error::Error::source(&error), |cause| cause.source())
                .for_each(|cause| eprintln!("  caused by: {cause}"));
            ExitCode::FAILURE
        }
    }
}
