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
                        &short_url.last_modified.to_string(),
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

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use shorty::{
        repository::{
            Repository, WritableRepository,
            sqlite::{open_readonly_repository, open_writable_repository},
        },
        types::{ShortUrlName, Url},
    };
    use tempfile::tempdir;

    fn base_command() -> assert_cmd::Command {
        assert_cmd::Command::cargo_bin("shorty").unwrap()
    }

    fn migrate(db_path: &PathBuf) {
        let mut cmd = base_command();
        cmd.arg("migrate");
        cmd.arg("--database");
        cmd.arg(db_path);
        cmd.assert().success();
    }

    fn get(db_path: &PathBuf, name: &ShortUrlName) -> assert_cmd::Command {
        let mut cmd = base_command();
        cmd.arg("get");
        cmd.arg("--database");
        cmd.arg(db_path);
        cmd.arg(name.to_string());
        cmd
    }

    fn set(db_path: &PathBuf, name: &ShortUrlName, url: &Url) -> assert_cmd::Command {
        let mut cmd = base_command();
        cmd.arg("set");
        cmd.arg("--database");
        cmd.arg(db_path);
        cmd.arg(name.to_string());
        cmd.arg(url.to_string());
        cmd
    }

    fn list(db_path: &PathBuf) -> assert_cmd::Command {
        let mut cmd = base_command();
        cmd.arg("list");
        cmd.arg("--database");
        cmd.arg(db_path);
        cmd
    }

    fn export(db_path: &PathBuf) -> assert_cmd::Command {
        let mut cmd = base_command();
        cmd.arg("export");
        cmd.arg("--database");
        cmd.arg(db_path);
        cmd
    }

    #[test]
    fn test_migrate() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        assert!(!db_path.exists());

        migrate(&db_path);

        assert!(db_path.exists());

        let repo = open_readonly_repository(&db_path).unwrap();
        assert!(repo.has_latest_migrations().unwrap());
    }

    #[test]
    fn test_get() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        assert!(!db_path.exists());

        let name = "aa".try_into().unwrap();
        let url: Url = "https://example.com".try_into().unwrap();
        let mut repo = open_writable_repository(&db_path).unwrap();
        repo.migrate().unwrap();
        repo.insert_url(&name, &url).unwrap();

        let mut cmd = get(&db_path, &name);
        cmd.assert().success().stdout(format!("{url}\n"));
    }

    #[test]
    fn test_set() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        assert!(!db_path.exists());
        migrate(&db_path);

        let name = "aa".try_into().unwrap();
        let url = "https://example.com".try_into().unwrap();

        let mut cmd = set(&db_path, &name, &url);
        cmd.assert().success();

        let repo = open_readonly_repository(&db_path).unwrap();
        let short_url = repo.get_url(&name).unwrap();
        assert!(short_url.is_some());
        let short_url = short_url.unwrap();
        assert_eq!(name, short_url.name);
        assert_eq!(url, short_url.url);
    }

    #[test]
    fn test_list() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        assert!(!db_path.exists());

        let name = "aa".try_into().unwrap();
        let url: Url = "https://example.com".try_into().unwrap();
        let mut repo = open_writable_repository(&db_path).unwrap();
        repo.migrate().unwrap();
        repo.insert_url(&name, &url).unwrap();

        let mut cmd = list(&db_path);
        cmd.assert().success().stdout(format!("{name}\n"));
    }

    #[test]
    fn test_export() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        assert!(!db_path.exists());

        let name = "aa".try_into().unwrap();
        let url: Url = "https://example.com".try_into().unwrap();
        let mut repo = open_writable_repository(&db_path).unwrap();
        repo.migrate().unwrap();
        repo.insert_url(&name, &url).unwrap();
        let short_url = repo.get_url(&name).unwrap().unwrap();

        let mut cmd = export(&db_path);
        let expected = format!(
            "shorturl,url,last_modified\r\n{},{},{}\r\n",
            short_url.name, short_url.url, short_url.last_modified
        );
        cmd.assert().success().stdout(expected);
    }
}
