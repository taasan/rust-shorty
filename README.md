# shorty

A simple url shortener written in rust.

## Database migrations

`shorty --migrate path/to/config.toml`

## Apache configuration

Read the [CGI howto](https://httpd.apache.org/docs/1.4/howto/cgi.html)

Create a web directory to store the CGI script and support files.
Copy the [./static](./static) directory to the web directory.

Create an executable file `index.cgi` with the correct
[shebang](https://en.wikipedia.org/wiki/Shebang_(Unix))

```toml
#!/path/to/executable

# Configuration in TOML format
database_file = "/path/to/sqlite/database"

[sentry]
enabled = false
dsn = "https://sentry@example.com/123"
```

If Apache is configured to use
[suexec](https://httpd.apache.org/docs/2.4/suexec.html), the file must
be owned by the user and have the correct permissions:

```shell
chmod 750 index.cgi
```

Create a `.htaccess` file to redirect all requests to the CGI script.

```apacheconf
AcceptPathInfo On

<IfModule mod_rewrite.c>
    RewriteEngine On
    RewriteCond %{REQUEST_FILENAME} !-f
    RewriteCond %{REQUEST_FILENAME} !-d
    RewriteRule "^(.*)" "index.cgi/$1" [QSA,L,NS]
</IfModule>
```
