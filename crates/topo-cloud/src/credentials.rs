//! Where the token for a server comes from: the `TOPO_TOKEN` environment
//! variable, or the file `topo login` writes.
//!
//! The file is `credentials.toml` in `$XDG_CONFIG_HOME/topo`, `%APPDATA%\topo`,
//! or `~/.config/topo`, readable only by its owner. It maps a server URL to
//! `{ token, id }`.

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Credential {
    pub token: String,
    /// Id of the token on the server, to revoke it on sign-out.
    pub id: String,
}

fn path() -> Result<PathBuf, Error> {
    let dir = ["XDG_CONFIG_HOME", "APPDATA"]
        .iter()
        .find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| Error::file(&PathBuf::from("credentials.toml"), "no home directory to keep it in"))?;
    Ok(dir.join("topo").join("credentials.toml"))
}

fn read() -> Result<toml::Table, Error> {
    let path = path()?;
    match std::fs::read_to_string(&path) {
        Ok(text) => text.parse().map_err(|e| Error::file(&path, e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(toml::Table::new()),
        Err(e) => Err(Error::file(&path, e)),
    }
}

/// The credential `topo login` stored for a server.
pub fn load(url: &str) -> Result<Option<Credential>, Error> {
    let path = path()?;
    let Some(value) = read()?.remove(url) else {
        return Ok(None);
    };
    value.try_into().map(Some).map_err(|e| Error::file(&path, e))
}

/// The token to send to a server. The environment variable is used as it is:
/// in a sandbox it may hold a placeholder that a proxy replaces on the way out.
pub fn token(url: &str) -> Result<String, Error> {
    if let Ok(token) = std::env::var("TOPO_TOKEN") {
        return Ok(token);
    }
    load(url)?.map(|credential| credential.token).ok_or_else(|| Error::NotSignedIn(url.to_owned()))
}

/// Stores or removes the credential for a server.
pub fn store(url: &str, credential: Option<&Credential>) -> Result<(), Error> {
    let path = path()?;
    let mut table = read()?;
    match credential {
        Some(credential) => {
            let value = toml::Value::try_from(credential).expect("a credential serializes to TOML");
            table.insert(url.to_owned(), value);
        }
        None => {
            table.remove(url);
        }
    }
    let dir = path.parent().expect("the file is in a directory");
    std::fs::create_dir_all(dir).map_err(|e| Error::file(dir, e))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&path).map_err(|e| Error::file(&path, e))?;
    file.write_all(table.to_string().as_bytes()).map_err(|e| Error::file(&path, e))
}
