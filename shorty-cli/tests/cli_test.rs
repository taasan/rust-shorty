#[cfg(test)]
#[serial_test::serial]
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
        assert_cmd::cargo::cargo_bin_cmd!("shorty")
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
            short_url.name,
            short_url.url,
            short_url
                .last_modified
                .map_or(String::new(), |x| x.to_string())
        );
        cmd.assert().success().stdout(expected);
    }
}
