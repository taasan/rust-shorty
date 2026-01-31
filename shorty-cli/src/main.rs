use anyhow::anyhow;
use core::cell::RefCell;
use shorty::anyhow;
use std::io::Write as _;
use std::path::PathBuf;

use clap::Parser;
use csv::{Terminator, WriterBuilder};
use git_version::git_version;
use shorty::{
    repository::{
        Repository, WritableRepository,
        sqlite::{open_readonly_repository, open_writable_repository},
    },
    types::{ShortUrlName, Url},
};

#[derive(Debug, Parser)] // requires `derive` feature
#[command(about = "Shorty", long_about = None, version = git_version!())]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Args, Clone)]
struct CommonArgs {
    #[arg(long, env = "SHORTY_DB")]
    database: PathBuf,
}

#[derive(Debug, clap::Parser)]
enum Command {
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
    List {
        #[command(flatten)]
        common: CommonArgs,
    },
    Export {
        #[command(flatten)]
        common: CommonArgs,
    },
    Migrate {
        #[command(flatten)]
        common: CommonArgs,
    },
}

impl Command {
    fn execute(self) -> Result<(), anyhow::Error> {
        match self {
            Self::Set { name, url, common } => {
                let mut repo = open_writable_repository(common.database)?;
                if !repo.has_latest_migrations()? {
                    return Err(anyhow!("migrations needed"));
                }
                repo.insert_url(&name, &url)?;
                eprintln!("url saved");
                Ok(())
            }
            Self::Get { name, common } => {
                let repo = open_readonly_repository(common.database)?;
                let out = RefCell::new(std::io::stdout().lock());
                match repo.get_url(&name)? {
                    Some(url) => {
                        writeln!(*out.borrow_mut(), "{}", url.url)?;
                        Ok(())
                    }
                    None => Err(anyhow!("url not found")),
                }
            }
            Self::List { common } => {
                let repo = open_readonly_repository(common.database)?;
                let out = RefCell::new(std::io::stdout().lock());
                repo.for_each_name(&|name| Ok(writeln!(*out.borrow_mut(), "{name}")?))?;
                Ok(())
            }
            Self::Export { common } => {
                let repo = open_readonly_repository(common.database)?;
                let wtr = RefCell::new(
                    WriterBuilder::new()
                        .terminator(Terminator::CRLF)
                        .from_writer(std::io::stdout()),
                );
                (*wtr.borrow_mut()).write_record(["shorturl", "url", "last_modified"])?;
                repo.for_each_short_url(&|short_url| {
                    (*wtr.borrow_mut()).write_record([
                        &short_url.name.to_string(),
                        &short_url.url.to_string(),
                        &short_url
                            .last_modified
                            .map_or(String::new(), |x| x.to_string()),
                    ])?;
                    (*wtr.borrow_mut()).flush()?;
                    Ok(())
                })?;
                Ok(())
            }
            Self::Migrate { common } => {
                let mut repo = open_writable_repository(common.database)?;
                repo.migrate()
            }
        }
    }
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    cli.command.execute()
}
