//! The one thing that touches the network, behind a trait (T2.2.1).
//!
//! [`UreqTransport`] makes real requests; [`FakeTransport`] replays scripted
//! responses and records every request, which is how the EXT tests prove
//! what was and was not asked for without a network.
//!
//! A transport does **no** policy: no delays, no guards, no retries, and it
//! never follows redirects on its own. All of that lives in
//! [`crate::client`], so there is exactly one place to audit.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::time::Duration;

use crate::url::WikiUrl;

/// A response, reduced to what the client needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    /// Header names are stored lower-case.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Response {
    /// A response with a status and body and no headers.
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Response {
            status,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    /// The same response with one more header.
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers
            .push((name.to_ascii_lowercase(), value.to_owned()));
        self
    }

    /// A header's value, by case-insensitive name.
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// A request that produced no HTTP response at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportError(pub String);

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "network error: {}", self.0)
    }
}

impl std::error::Error for TransportError {}

/// Something that can perform a GET.
pub trait Transport {
    /// Performs one GET with extra headers. Never follows redirects.
    fn get(&self, url: &WikiUrl, headers: &[(&str, String)]) -> Result<Response, TransportError>;
}

/// Real requests, through `ureq`.
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    /// A transport sending the project User-Agent (EXT-4).
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .user_agent(crate::user_agent())
            // Redirects are the client's decision, because a redirect target
            // has to pass both URL guards before it is requested (T2.2.4).
            .max_redirects(0)
            // A 404 or 503 is information, not an error to unwind through.
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build();
        UreqTransport {
            agent: config.into(),
        }
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        UreqTransport::new()
    }
}

impl Transport for UreqTransport {
    fn get(&self, url: &WikiUrl, headers: &[(&str, String)]) -> Result<Response, TransportError> {
        let mut request = self.agent.get(url.as_str());
        for (name, value) in headers {
            request = request.header(*name, value.as_str());
        }
        let mut response = request
            .call()
            .map_err(|error| TransportError(error.to_string()))?;

        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_ascii_lowercase(),
                    value.to_str().unwrap_or("").to_owned(),
                )
            })
            .collect();
        let body = response
            .body_mut()
            .with_config()
            // The largest page the crawl reads is about 1 MB (T2.1 §2).
            .limit(16 * 1024 * 1024)
            .read_to_string()
            .map_err(|error| TransportError(error.to_string()))?;

        Ok(Response {
            status,
            headers,
            body,
        })
    }
}

/// One request a [`FakeTransport`] saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

/// Scripted responses, for tests.
///
/// Responses are queued per URL and served in order; a URL with nothing
/// queued answers 404. Every request is recorded, including ones that were
/// answered from the default.
#[derive(Debug, Default)]
pub struct FakeTransport {
    scripted: RefCell<BTreeMap<String, VecDeque<Result<Response, TransportError>>>>,
    requests: RefCell<Vec<RecordedRequest>>,
}

impl FakeTransport {
    /// An empty script.
    pub fn new() -> Self {
        FakeTransport::default()
    }

    /// Queues a response for a URL.
    pub fn respond(&self, url: &str, response: Response) -> &Self {
        self.scripted
            .borrow_mut()
            .entry(url.to_owned())
            .or_default()
            .push_back(Ok(response));
        self
    }

    /// Queues a network failure for a URL.
    pub fn fail(&self, url: &str, message: &str) -> &Self {
        self.scripted
            .borrow_mut()
            .entry(url.to_owned())
            .or_default()
            .push_back(Err(TransportError(message.to_owned())));
        self
    }

    /// Queues the permissive `robots.txt` the real wiki serves.
    pub fn with_wiki_robots(self) -> Self {
        self.respond(
            crate::url::ROBOTS_URL,
            Response::new(200, crate::robots::WIKI_ROBOTS_TXT),
        );
        self
    }

    /// Every request so far, in order.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.borrow().clone()
    }

    /// Every URL requested so far, in order.
    pub fn urls(&self) -> Vec<String> {
        self.requests
            .borrow()
            .iter()
            .map(|r| r.url.clone())
            .collect()
    }
}

impl Transport for FakeTransport {
    fn get(&self, url: &WikiUrl, headers: &[(&str, String)]) -> Result<Response, TransportError> {
        self.requests.borrow_mut().push(RecordedRequest {
            url: url.as_str().to_owned(),
            headers: headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
        });
        self.scripted
            .borrow_mut()
            .get_mut(url.as_str())
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| Ok(Response::new(404, "")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gwsim_data::WikiTitle;

    #[test]
    fn a_fake_transport_serves_its_script_in_order_and_records_requests() {
        let transport = FakeTransport::new();
        let url = WikiUrl::article(&WikiTitle("Energy Surge".to_owned()));
        transport
            .respond(url.as_str(), Response::new(503, ""))
            .respond(url.as_str(), Response::new(200, "page"));

        let first = transport.get(&url, &[]).unwrap();
        let second = transport
            .get(&url, &[("If-Modified-Since", "x".to_owned())])
            .unwrap();
        let third = transport.get(&url, &[]).unwrap();

        assert_eq!(first.status, 503);
        assert_eq!(second.body, "page");
        assert_eq!(third.status, 404, "an exhausted script answers 404");
        assert_eq!(transport.requests().len(), 3);
        assert_eq!(transport.requests()[1].headers[0].0, "If-Modified-Since");
    }

    #[test]
    fn headers_are_found_whatever_their_case() {
        let response = Response::new(200, "").with_header("Last-Modified", "then");
        assert_eq!(response.header("last-modified"), Some("then"));
        assert_eq!(response.header("LAST-MODIFIED"), Some("then"));
        assert_eq!(response.header("etag"), None);
    }
}
