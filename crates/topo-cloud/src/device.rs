//! GitHub's OAuth device flow: the user approves in a browser, on any device,
//! and this process polls for the access token.

use std::time::Duration;

use serde::Deserialize;

use crate::Error;

pub const GITHUB: &str = "https://github.com";

/// What the user has to do: open `verification_uri` and enter `user_code`.
#[derive(Debug, Deserialize)]
pub struct Prompt {
    pub user_code: String,
    pub verification_uri: String,
    device_code: String,
    /// Seconds to wait between polls.
    interval: u64,
}

#[derive(Deserialize)]
struct Poll {
    access_token: Option<String>,
    error: Option<String>,
}

/// Runs the device flow of the OAuth app `client_id` and returns a GitHub
/// access token. `show` is called once, with what to tell the user.
pub fn sign_in(github: &str, client_id: &str, show: impl FnOnce(&Prompt)) -> Result<String, Error> {
    let agent = ureq::agent();
    let post = |path: &str, form: &[(&str, &str)]| -> Result<ureq::http::Response<ureq::Body>, Error> {
        let url = format!("{github}{path}");
        agent
            .post(&url)
            .header("accept", "application/json")
            .send_form(form.iter().copied())
            .map_err(|source| Error::Http { url, source })
    };
    let json = |e: ureq::Error| Error::SignIn(e.to_string());

    let prompt: Prompt =
        post("/login/device/code", &[("client_id", client_id)])?.body_mut().read_json().map_err(json)?;
    show(&prompt);
    let mut interval = prompt.interval;
    loop {
        std::thread::sleep(Duration::from_secs(interval));
        let form = [
            ("client_id", client_id),
            ("device_code", prompt.device_code.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ];
        let poll: Poll = post("/login/oauth/access_token", &form)?.body_mut().read_json().map_err(json)?;
        match (poll.access_token, poll.error.as_deref()) {
            (Some(token), _) => return Ok(token),
            (None, Some("authorization_pending")) => {}
            // GitHub asks for five more seconds between polls.
            (None, Some("slow_down")) => interval += 5,
            (None, error) => return Err(Error::SignIn(error.unwrap_or("GitHub returned no token").to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polls_until_the_user_approves() {
        let mut server = mockito::Server::new();
        let code = r#"{"device_code":"d","user_code":"ABCD-1234","verification_uri":"https://github.com/login/device","expires_in":900,"interval":0}"#;
        let start = server.mock("POST", "/login/device/code").with_body(code).create();
        // Of the mocks that match, the oldest one still expecting requests answers.
        let mut token =
            |body: &str, times| server.mock("POST", "/login/oauth/access_token").with_body(body).expect(times).create();
        let pending = token(r#"{"error":"authorization_pending"}"#, 2);
        let approved = token(r#"{"access_token":"gho_x"}"#, 1);

        let mut shown = String::new();
        let result = sign_in(&server.url(), "client", |p| shown = format!("{} {}", p.verification_uri, p.user_code));
        assert_eq!(result.unwrap(), "gho_x");
        assert_eq!(shown, "https://github.com/login/device ABCD-1234");
        start.assert();
        pending.assert();
        approved.assert();
    }

    #[test]
    fn stops_when_the_user_refuses() {
        let mut server = mockito::Server::new();
        let code = r#"{"device_code":"d","user_code":"A","verification_uri":"u","interval":0}"#;
        server.mock("POST", "/login/device/code").with_body(code).create();
        server.mock("POST", "/login/oauth/access_token").with_body(r#"{"error":"access_denied"}"#).create();
        let error = sign_in(&server.url(), "client", |_| {}).unwrap_err();
        assert_eq!(error.to_string(), "GitHub sign-in failed: access_denied");
    }
}
