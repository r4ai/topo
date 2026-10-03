//! Where the token for a server comes from: the `TOPO_TOKEN` environment
//! variable, or the file `topo login` writes.
//!
//! The file is `credentials.toml` in `$XDG_CONFIG_HOME/topo`, `%APPDATA%\topo`,
//! or `~/.config/topo`, readable only by its owner. It maps a server URL to
//! `{ token, id }`.

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
    match topo_core::files::read(path.parent().expect("credentials have a directory"), "credentials.toml") {
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

/// The environment token is sent verbatim, including proxy placeholders, but
/// only to the server explicitly bound by `TOPO_CLOUD_URL` outside the workspace.
pub fn token(url: &str) -> Result<String, Error> {
    if let Ok(token) = std::env::var("TOPO_TOKEN") {
        let trusted = std::env::var("TOPO_CLOUD_URL").ok();
        check_destination(url, trusted.as_deref())?;
        return Ok(token);
    }
    load(url)?.map(|credential| credential.token).ok_or_else(|| Error::NotSignedIn(url.to_owned()))
}

fn check_destination(url: &str, trusted: Option<&str>) -> Result<(), Error> {
    match trusted {
        Some(trusted) if !trusted.is_empty() && trusted.trim_end_matches('/') == url.trim_end_matches('/') => Ok(()),
        _ => Err(Error::CredentialDestination),
    }
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
    topo_core::files::write(dir, "credentials.toml", &table.to_string(), true).map_err(|e| Error::file(&path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_credentials_require_an_external_destination_binding() {
        let trusted = "https://trusted.example/base";
        for destination in [
            "https://evil.example/base",
            "https://trusted.example.evil/base",
            "http://trusted.example/base",
            "https://trusted.example/other",
            "https://trusted.example@evil.example/base",
        ] {
            assert!(matches!(check_destination(destination, Some(trusted)), Err(Error::CredentialDestination)));
        }
        assert!(check_destination(trusted, None).is_err());
        assert!(check_destination(trusted, Some("")).is_err());
        assert!(check_destination(&format!("{trusted}/"), Some(trusted)).is_ok());
    }
}
