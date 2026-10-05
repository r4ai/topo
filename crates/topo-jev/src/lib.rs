//! Client for Jev-compatible "System One" decision models (`POST /v1/systemone`)
//! and the graph-organizing operations built on them.
//!
//! Works with the hosted Jev API and with local servers that speak the same
//! wire format, such as `decider` (`scripts/serve.sh Mapika/decider-4b 8000`).

pub mod organize;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {message}")]
    Config { path: String, message: String },
    #[error("request to {url} failed (is a Jev-compatible server running? see `jev.base_url` in .topo/config.toml)")]
    Http {
        url: String,
        #[source]
        source: ureq::Error,
    },
    #[error("server returned no usable `{field}` for question `{question}`")]
    MissingAnswer { question: String, field: &'static str },
}

/// `[jev]` table of `.topo/config.toml`. The file is optional.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "Config::default_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Minimum probability for a decision to become a proposal.
    #[serde(default = "Config::default_threshold")]
    pub threshold: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self { base_url: Self::default_base_url(), api_key: None, model: None, threshold: Self::default_threshold() }
    }
}

impl Config {
    fn default_base_url() -> String {
        "http://127.0.0.1:8000".into()
    }

    fn default_threshold() -> f64 {
        0.7
    }

    /// Reads the `[jev]` table of `<topo_dir>/config.toml`, or the defaults when
    /// the file or the table is absent.
    pub fn load(topo_dir: &Path) -> Result<Self, Error> {
        #[derive(Deserialize)]
        struct File {
            #[serde(default)]
            jev: Option<Config>,
        }
        let path = topo_dir.join("config.toml");
        let err = |message: String| Error::Config { path: path.display().to_string(), message };
        match topo_core::files::read(topo_dir, "config.toml") {
            Ok(text) => Ok(toml::from_str::<File>(&text).map_err(|e| err(e.to_string()))?.jev.unwrap_or_default()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(err(e.to_string())),
        }
    }

    /// Whether the workspace opted into a Jev model: its `config.toml` has a `[jev]` table.
    /// An unreadable or malformed file counts as not configured, so the GUI hides its controls.
    pub fn configured(topo_dir: &Path) -> bool {
        #[derive(Deserialize)]
        struct File {
            #[serde(default)]
            jev: Option<toml::Value>,
        }
        match topo_core::files::read(topo_dir, "config.toml") {
            Ok(text) => toml::from_str::<File>(&text).is_ok_and(|file| file.jev.is_some()),
            Err(_) => false,
        }
    }
}

/// A typed question. Each variant maps to one Jev question type.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Pick one of the named options; values describe each option.
    Choice { instructions: String, criteria: BTreeMap<String, String> },
    /// Place the state on an ordered scale, lowest first.
    Score { instructions: String, criteria: Vec<String> },
    /// Yes/no; the answer is the probability of "yes".
    Noul { instructions: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Answer {
    choice: Option<String>,
    probabilities: Option<BTreeMap<String, f64>>,
    noul: Option<f64>,
    score: Option<f64>,
}

impl Answer {
    fn field<T: Clone>(value: &Option<T>, question: &str, field: &'static str) -> Result<T, Error> {
        value.clone().ok_or_else(|| Error::MissingAnswer { question: question.into(), field })
    }
}

/// Answers keyed by question name. Accessors fail if the server left one out.
pub struct Answers(BTreeMap<String, Answer>);

impl Answers {
    fn get(&self, question: &str, field: &'static str) -> Result<&Answer, Error> {
        self.0.get(question).ok_or_else(|| Error::MissingAnswer { question: question.into(), field })
    }

    /// Probability of "yes".
    pub fn noul(&self, question: &str) -> Result<f64, Error> {
        Answer::field(&self.get(question, "noul")?.noul, question, "noul")
    }

    /// Chosen option and its probability.
    pub fn choice(&self, question: &str) -> Result<(String, f64), Error> {
        let answer = self.get(question, "choice")?;
        let choice = Answer::field(&answer.choice, question, "choice")?;
        let p = Answer::field(&answer.probabilities, question, "probabilities")?
            .get(&choice)
            .copied()
            .ok_or_else(|| Error::MissingAnswer { question: question.into(), field: "probabilities" })?;
        Ok((choice, p))
    }

    /// Expected position on the scale, normalized to 0..=1.
    pub fn score(&self, question: &str) -> Result<f64, Error> {
        Answer::field(&self.get(question, "score")?.score, question, "score")
    }
}

pub struct Client {
    config: Config,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(config: Config) -> Self {
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(300))).build().into();
        Self { config, agent }
    }

    pub fn threshold(&self) -> f64 {
        self.config.threshold
    }

    /// Asks every question about one `state` in a single pass.
    pub fn decide(&self, state: Value, questions: BTreeMap<String, Question>) -> Result<Answers, Error> {
        #[derive(Serialize)]
        struct Request<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            model: Option<&'a str>,
            state: Value,
            questions: BTreeMap<String, Question>,
        }
        #[derive(Deserialize)]
        struct Response {
            answers: BTreeMap<String, Answer>,
        }
        let url = format!("{}/v1/systemone", self.config.base_url.trim_end_matches('/'));
        let http = |source| Error::Http { url: url.clone(), source };
        let mut request = self.agent.post(&url);
        if let Some(key) = &self.config.api_key {
            request = request.header("Authorization", format!("Bearer {key}"));
        }
        let body = Request { model: self.config.model.as_deref(), state, questions };
        let response: Response = request.send_json(&body).map_err(http)?.body_mut().read_json().map_err(http)?;
        Ok(Answers(response.answers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "topo-jev-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn configured_needs_a_jev_table_in_the_workspace_config() {
        let dir = temp();
        assert!(!Config::configured(&dir), "a missing config file is not configured");
        fs::write(dir.join("config.toml"), "[cloud]\nurl = \"https://topo.example\"\n").unwrap();
        assert!(!Config::configured(&dir), "a config without a [jev] table is not configured");
        fs::write(dir.join("config.toml"), "[jev]\nbase_url = \"http://127.0.0.1:8000\"\n").unwrap();
        assert!(Config::configured(&dir), "an explicit [jev] table is configured");
        fs::write(dir.join("config.toml"), "[jev\n").unwrap();
        assert!(!Config::configured(&dir), "a malformed config is not configured");
        fs::remove_dir_all(&dir).unwrap();
    }
}
