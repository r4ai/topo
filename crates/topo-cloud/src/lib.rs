//! Client of the topo cloud API: opens a workspace whose nodes a server holds.
//!
//! A `.topo` directory is linked to the cloud by a `[cloud]` table in its
//! `config.toml`. [`open`] returns a [`Workspace`] either way, so callers
//! mutate `graph` and `save()` without knowing where the nodes live.

pub mod client;
pub mod config;
pub mod credentials;
pub mod device;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use topo_core::Workspace;

pub use client::{Client, HttpRemote};
pub use config::CloudConfig;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {message}")]
    File { path: PathBuf, message: String },
    #[error("not signed in to {0} (run `topo login`, or set TOPO_TOKEN)")]
    NotSignedIn(String),
    #[error("request to {url} failed")]
    Http {
        url: String,
        #[source]
        source: ureq::Error,
    },
    /// The server refused the request. `code` is the stable identifier of the reason.
    #[error("{message}")]
    Api { status: u16, code: String, message: String },
    #[error("GitHub sign-in failed: {0}")]
    SignIn(String),
    #[error(transparent)]
    Core(#[from] topo_core::Error),
}

impl Error {
    fn file(path: &Path, message: impl ToString) -> Self {
        Error::File { path: path.to_owned(), message: message.to_string() }
    }
}

/// Opens a `.topo` directory: over the cloud if it is linked, otherwise from its files.
pub fn open(dir: PathBuf) -> Result<Workspace, Error> {
    let Some(cloud) = config::load(&dir)? else {
        return Ok(Workspace::open(dir)?);
    };
    let client = Client::new(&cloud.url, credentials::token(&cloud.url)?);
    Ok(Workspace::open_remote(dir, Arc::new(HttpRemote::new(client, cloud.workspace)))?)
}

/// Opens the nearest `.topo` directory in `start` or its ancestors.
pub fn discover(start: &Path) -> Result<Workspace, Error> {
    open(Workspace::find_dir(start)?)
}
