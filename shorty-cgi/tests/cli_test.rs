#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[serial_test::serial]
mod test {
    use std::{
        fs::File,
        io::Write as _,
        os::unix::fs::PermissionsExt as _,
        path::{self, Path, PathBuf},
    };

    use assert_cmd::cargo::cargo_bin_cmd;
    use cgi::Config;
    use predicates::prelude::*;
    use shorty::{
        repository::{sqlite::open_writable_repository, WritableRepository},
        types::{ShortUrlName, Url},
    };
    use tempfile::{tempdir, TempDir};

    fn base_command(db_path: &Path) -> assert_cmd::Command {
        let temp_dir = db_path.parent().unwrap();
        let cgi_path = PathBuf::from(cargo_bin_cmd!("cgi").get_program());

        let script_path = temp_dir.join("shorty.cgi");
        let config = Config {
            database_file: db_path.to_path_buf(),
            #[cfg(feature = "sentry")]
            sentry: None,
        };
        let toml_string = toml::to_string_pretty(&config).unwrap();
        let script = format!(
            "\
 #!{}

 {toml_string}
 ",
            cgi_path.display()
        );
        let mut file = File::create(&script_path).unwrap();
        file.write_all(script.as_bytes()).unwrap();
        file.sync_all().unwrap();
        let mut perms = ::std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
        assert_cmd::Command::new(script_path.to_str().unwrap())
    }

    fn init_repo() -> (
        impl shorty::repository::WritableRepository,
        TempDir,
        path::PathBuf,
    ) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        assert!(!db_path.exists());

        let mut repo = open_writable_repository(db_path.clone()).unwrap();
        repo.migrate().unwrap();
        (repo, temp_dir, db_path)
    }

    fn get(db_path: &Path, name: Option<&ShortUrlName>) -> assert_cmd::Command {
        let mut cmd = base_command(db_path);
        cmd.env("GATEWAY_INTERFACE", "CGI/1.1")
            .env("REQUEST_METHOD", "GET")
            .env("REQUEST_SCHEME", "http")
            .env("REQUEST_URI", "/")
            .env("SERVER_NAME", "localhost.localdomain")
            .env("SERVER_PROTOCOL", "HTTP/1.0");
        if let Some(name) = name {
            cmd.env("PATH_INFO", format!("/{name}"));
        }
        cmd
    }

    #[test]
    fn test_get() {
        let (mut repo, _temp_dir, db_path) = init_repo();

        let name = "short-url".try_into().unwrap();
        let url: Url = "https://example.com".try_into().unwrap();
        repo.insert_url(&name, &url).unwrap();

        let mut cmd = get(&db_path, Some(&name));
        cmd.assert()
            .success()
            .stdout(predicate::str::starts_with("Status: 200"))
            .stdout(predicate::str::contains(name.to_string()))
            .stdout(predicate::str::contains(url.to_string()));
    }

    #[test]
    fn test_get_404() {
        let (mut _repo, _temp_dir, db_path) = init_repo();

        let name: ShortUrlName = "short-url".try_into().unwrap();
        let mut cmd = get(&db_path, Some(&name));
        cmd.assert()
            .success()
            .stdout(predicate::str::starts_with("Status: 404"));
    }

    #[test]
    fn test_get_landing_page() {
        let (mut _repo, _temp_dir, db_path) = init_repo();

        let mut cmd = get(&db_path, None);
        cmd.assert()
            .success()
            .stdout(predicate::str::starts_with("Status: 200"))
            .stdout(predicate::str::contains("Douglas Adams"));
    }
}
