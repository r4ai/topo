//! The `[cloud]` table of `.topo/config.toml`, which links a workspace to a server.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudConfig {
    /// Base URL of the server, such as `https://api.example.com`.
    pub url: String,
    /// Id of the workspace on that server.
    pub workspace: String,
}

fn read(topo_dir: &Path) -> Result<toml::Table, Error> {
    let path = topo_dir.join("config.toml");
    match std::fs::read_to_string(&path) {
        Ok(text) => text.parse().map_err(|e| Error::file(&path, e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(toml::Table::new()),
        Err(e) => Err(Error::file(&path, e)),
    }
}

/// The link of this workspace, if it has one.
pub fn load(topo_dir: &Path) -> Result<Option<CloudConfig>, Error> {
    let Some(table) = read(topo_dir)?.remove("cloud") else {
        return Ok(None);
    };
    table.try_into().map(Some).map_err(|e| Error::file(&topo_dir.join("config.toml"), e))
}

/// Sets or removes the link, keeping the other tables of the file. Comments are not kept.
pub fn store(topo_dir: &Path, cloud: Option<&CloudConfig>) -> Result<(), Error> {
    let path = topo_dir.join("config.toml");
    let mut table = read(topo_dir)?;
    match cloud {
        Some(cloud) => {
            let value = toml::Value::try_from(cloud).expect("the link serializes to TOML");
            table.insert("cloud".into(), value);
        }
        None => {
            table.remove("cloud");
        }
    }
    std::fs::write(&path, table.to_string()).map_err(|e| Error::file(&path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_is_stored_beside_other_tables() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()).unwrap(), None);
        std::fs::write(dir.path().join("config.toml"), "[jev]\nthreshold = 0.5\n").unwrap();
        let link = CloudConfig { url: "https://example.com".into(), workspace: "w1".into() };
        store(dir.path(), Some(&link)).unwrap();
        assert_eq!(load(dir.path()).unwrap(), Some(link));
        store(dir.path(), None).unwrap();
        assert_eq!(std::fs::read_to_string(dir.path().join("config.toml")).unwrap(), "[jev]\nthreshold = 0.5\n");

        std::fs::write(dir.path().join("config.toml"), "[cloud]\nurl = \"u\"\n").unwrap();
        assert!(load(dir.path()).unwrap_err().to_string().contains("missing field `workspace`"));
    }
}
