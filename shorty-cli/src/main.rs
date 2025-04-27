use anyhow::anyhow;
use std::io::Write as _;
use std::path::PathBuf;

use clap::Parser;
use shorty::{
    repository::{
        Repository, WritableRepository, open_sqlite3_repository, open_writable_sqlite3_repository,
    },
    types::{ShortUrlName, Url},
};

#[derive(Debug, Parser)] // requires `derive` feature
#[command(about = "Shorty", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, clap::Args, Clone)]
struct CommonArgs {
    #[arg(long, env = "SHORTY_DB")]
    database: PathBuf,
}

#[derive(Debug, clap::Parser)]
enum Commands {
    Set {
        //
        #[arg(value_parser = |s: &str| ShortUrlName::try_from(s))]
        name: ShortUrlName,
        #[arg(value_parser = |s: &str| Url::try_from(s))]
        url: Url,
        #[command(flatten)]
        common: CommonArgs,
    },
    Get {
        //
        #[arg(value_parser = |s: &str| ShortUrlName::try_from(s))]
        name: ShortUrlName,
        #[command(flatten)]
        common: CommonArgs,
    },
    Migrate {
        #[command(flatten)]
        common: CommonArgs,
    },
}

impl Cli {
    fn execute(self) -> Result<(), anyhow::Error> {
        match self.command {
            Commands::Set { name, url, common } => {
                let mut repo = open_writable_sqlite3_repository(common.database)?;
                if !repo.has_latest_migrations()? {
                    return Err(anyhow!("migrations needed"));
                }
                repo.insert_url(&name, &url)?;
                eprintln!("url saved");
                Ok(())
            }
            Commands::Get { name, common } => {
                let repo = open_sqlite3_repository(common.database)?;
                match repo.get_url(&name)? {
                    Some(url) => {
                        let mut out = std::io::stdout().lock();
                        writeln!(out, "{}", url.url)?;
                        Ok(())
                    }
                    None => Err(anyhow!("url not found")),
                }
            }
            Commands::Migrate { common } => {
                let mut repo = open_writable_sqlite3_repository(common.database)?;
                repo.migrate()
            }
        }
    }
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    cli.execute()
}
