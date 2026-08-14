//! Web fetch handler (E7.3). Fetches a URL via the transport and returns the
//! body as an `External` output. The executor tags every result `Tainted`, so
//! fetched web content is untrusted by construction and can never go on to drive
//! egress / credentials / memory (invariant 7, floored at build).
//!
//! **Egress policy (finding #12).** The handler used to fetch whatever URL the
//! spec carried, ignoring the spec's own `NetworkPolicy` — so a spec built with
//! `NetworkPolicy::Disabled` still made the request. Unlike `CommandHandler`,
//! which cannot police a subprocess's sockets and therefore fails closed (D47),
//! this handler *owns* the egress it performs, so it can and now does enforce the
//! policy itself: `Disabled` refuses, `AllowHosts` matches the URL's real host,
//! and loopback / link-local / private-range targets are refused unless the
//! allowlist names them explicitly.

use harness_types::{ExecutionSpec, NetworkPolicy};

use crate::handler::{ExecError, ExecOutput, Handler};
use crate::handlers::{str_field, structured};
use crate::transport::WebFetcher;

pub struct WebHandler {
    fetcher: Box<dyn WebFetcher>,
}

impl WebHandler {
    pub fn new(fetcher: Box<dyn WebFetcher>) -> Self {
        Self { fetcher }
    }
}

impl Handler for WebHandler {
    fn execute(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let url = str_field(structured(spec)?, "url")?;
        check_egress(&url, spec.network())?;
        let content = self.fetcher.fetch(&url).map_err(ExecError::Io)?;
        Ok(ExecOutput::External {
            source: format!("web:{url}"),
            content,
        })
    }

    fn simulate(&self, spec: &ExecutionSpec) -> Result<ExecOutput, ExecError> {
        let url = str_field(structured(spec)?, "url")?;
        // Simulation performs no fetch, but it reports what *would* happen — and
        // what would happen to a blocked URL is a refusal, so say so.
        check_egress(&url, spec.network())?;
        Ok(ExecOutput::Simulated(format!("would fetch {url}")))
    }
}

/// Decide whether `url` may be fetched under `policy`.
fn check_egress(url: &str, policy: &NetworkPolicy) -> Result<(), ExecError> {
    let blocked = |reason: &str| ExecError::NetworkBlocked {
        url: url.to_string(),
        reason: reason.to_string(),
    };

    let allowed = match policy {
        NetworkPolicy::Disabled => return Err(blocked("network policy is Disabled")),
        NetworkPolicy::AllowHosts(hosts) => hosts,
    };

    let host = host_of(url).ok_or_else(|| blocked("URL has no parseable host"))?;
    if host.is_empty() {
        return Err(blocked("URL has an empty host"));
    }

    let listed = allowed.iter().any(|h| host_matches(&host, h));
    if !listed {
        return Err(blocked(&format!("host {host} is not in the allowlist")));
    }
    // An allowlist entry naming a local target is an explicit choice and is
    // honoured; a *wildcard* entry must not silently reach the metadata service.
    if is_local_target(&host) && !allowed.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
        return Err(blocked(&format!(
            "host {host} is a loopback/link-local/private target and is not named explicitly"
        )));
    }
    Ok(())
}

/// The host of an absolute URL, lowercased, with userinfo and port removed.
///
/// Hand-rolled rather than pulling in a URL crate, so the parts that matter for a
/// security decision are visible and tested here. The one that matters most is
/// **userinfo**: `https://allowed.example@evil.example/` has host `evil.example`,
/// and a naive "does the URL contain an allowed host" check reads it as allowed.
fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    // Authority ends at the first '/', '?' or '#'.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    // Userinfo ends at the LAST '@' — an attacker may embed one in the userinfo.
    let hostport = match authority.rsplit_once('@') {
        Some((_userinfo, after)) => after,
        None => authority,
    };
    // IPv6 literals are bracketed and may contain colons.
    let host = if let Some(stripped) = hostport.strip_prefix('[') {
        stripped.split_once(']').map(|(h, _)| h)?
    } else {
        hostport.split(':').next().unwrap_or_default()
    };
    Some(host.trim_end_matches('.').to_ascii_lowercase())
}

/// Allowlist matching: exact host, or a `.suffix` entry matching subdomains.
/// A bare entry never matches a subdomain, so `example.com` does not admit
/// `evil-example.com` or `example.com.evil.net`.
fn host_matches(host: &str, entry: &str) -> bool {
    let entry = entry.trim_end_matches('.').to_ascii_lowercase();
    if let Some(suffix) = entry.strip_prefix('.') {
        return host == suffix || host.ends_with(&format!(".{suffix}"));
    }
    host == entry
}

/// Loopback, link-local (incl. the cloud metadata address), or RFC1918 private.
fn is_local_target(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    if let Ok(v6) = host.parse::<std::net::Ipv6Addr>() {
        if v6.is_loopback() || v6.is_unspecified() {
            return true;
        }
        // Unique-local (fc00::/7) and link-local (fe80::/10).
        let seg = v6.segments()[0];
        return (seg & 0xfe00) == 0xfc00 || (seg & 0xffc0) == 0xfe80;
    }
    let Ok(v4) = host.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local() // 169.254.0.0/16 — the metadata service lives here
        || v4.is_unspecified()
        || v4.is_broadcast()
        || v4.octets()[0] == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allow(hosts: &[&str]) -> NetworkPolicy {
        NetworkPolicy::AllowHosts(hosts.iter().map(|h| h.to_string()).collect())
    }

    #[test]
    fn disabled_policy_refuses_every_url() {
        let err = check_egress("https://docs.example/guide", &NetworkPolicy::Disabled)
            .expect_err("Disabled must refuse");
        assert!(matches!(err, ExecError::NetworkBlocked { .. }), "{err}");
    }

    #[test]
    fn allowlisted_host_is_permitted() {
        assert!(check_egress("https://docs.example/guide", &allow(&["docs.example"])).is_ok());
        // Port and path do not change the host.
        assert!(check_egress("https://docs.example:8443/x?y#z", &allow(&["docs.example"])).is_ok());
    }

    #[test]
    fn unlisted_host_is_refused() {
        let err = check_egress("https://evil.example/x", &allow(&["docs.example"]))
            .expect_err("unlisted host must be refused");
        assert!(format!("{err}").contains("not in the allowlist"), "{err}");
    }

    /// The bypass this parser exists for: userinfo that looks like an allowed host.
    #[test]
    fn userinfo_cannot_impersonate_an_allowed_host() {
        for url in [
            "https://docs.example@evil.example/x",
            "https://user:pass@evil.example/x",
            "https://docs.example@evil.example@evil2.example/x",
        ] {
            let err = check_egress(url, &allow(&["docs.example"]))
                .unwrap_err_or_else_msg("userinfo must not admit the host");
            assert!(err.contains("not in the allowlist"), "{url}: {err}");
        }
    }

    #[test]
    fn suffix_entries_match_subdomains_and_bare_entries_do_not() {
        assert!(check_egress("https://api.docs.example/x", &allow(&[".docs.example"])).is_ok());
        assert!(check_egress("https://docs.example/x", &allow(&[".docs.example"])).is_ok());
        // A bare entry is exact: neither a subdomain nor a lookalike matches.
        for url in [
            "https://api.docs.example/x",
            "https://evil-docs.example/x",
            "https://docs.example.evil.net/x",
        ] {
            assert!(
                check_egress(url, &allow(&["docs.example"])).is_err(),
                "{url} must not match a bare allowlist entry"
            );
        }
    }

    #[test]
    fn local_and_metadata_targets_need_naming_explicitly() {
        // A wildcard-ish suffix entry must not open the metadata service.
        for host in [
            "169.254.169.254",
            "127.0.0.1",
            "localhost",
            "10.0.0.5",
            "192.168.1.1",
            "0.0.0.0",
            "[::1]",
        ] {
            let url = format!("http://{host}/latest/meta-data/");
            let err = check_egress(&url, &allow(&[".", "example.com"]));
            assert!(
                err.is_err(),
                "{host} must not be reachable via a broad entry"
            );
        }
        // Naming it exactly is a deliberate choice and is honoured — a local dev
        // server is a legitimate target.
        assert!(check_egress("http://127.0.0.1:8080/x", &allow(&["127.0.0.1"])).is_ok());
    }

    #[test]
    fn a_url_without_a_host_is_refused() {
        for url in ["not-a-url", "file:///etc/passwd", "https:///x"] {
            assert!(
                check_egress(url, &allow(&["docs.example"])).is_err(),
                "{url}"
            );
        }
    }

    #[test]
    fn host_parsing_is_case_and_trailing_dot_insensitive() {
        assert_eq!(
            host_of("https://Docs.Example./x").as_deref(),
            Some("docs.example")
        );
        assert_eq!(host_of("https://[::1]:80/x").as_deref(), Some("::1"));
    }

    // Small helper so the userinfo test reads as one assertion per URL.
    trait UnwrapErrMsg {
        fn unwrap_err_or_else_msg(self, msg: &str) -> String;
    }
    impl UnwrapErrMsg for Result<(), ExecError> {
        fn unwrap_err_or_else_msg(self, msg: &str) -> String {
            match self {
                Ok(()) => panic!("{msg}"),
                Err(e) => format!("{e}"),
            }
        }
    }
}
