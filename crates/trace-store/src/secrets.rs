//! Value-level secret masking for the audit log (finding #17).
//!
//! [`crate::redact`] masks fields whose **key** matches a manifest pattern. That
//! catches `env.GITHUB_TOKEN`, and misses every secret that arrives inside an
//! ordinary value: a token in `command` (`curl -H 'Authorization: Bearer …'`), in
//! `url` (`?api_key=…`), or in a request `body`. Those keys cannot simply be
//! redacted wholesale — a trace whose every command reads `[REDACTED]` stops being
//! an audit log — so the value is scanned and only the secret-shaped span is
//! masked.
//!
//! The detectors are deliberately **high-confidence and few**. A redactor that
//! guesses produces two failures at once: it corrupts the audit record, and it
//! trains readers to ignore the mask. Every pattern here is a documented,
//! issuer-defined shape (`ghp_`, `AKIA`, `sk-`, a PEM header) or a syntactic
//! position that is a secret by definition (an `Authorization` header value, a
//! password in a URL's userinfo). Anything fuzzier — high-entropy string
//! heuristics, bare hex runs — is left out on purpose.
//!
//! This is a safety net under the manifest's own `observability.redact` patterns,
//! not a replacement for them: naming your secrets is still better than hoping a
//! scanner recognises them.

pub const REDACTED: &str = "[REDACTED]";

/// Mask secret-shaped spans inside a free-text value.
pub fn mask_secrets(text: &str) -> String {
    let out = mask_pem_blocks(text);
    let out = mask_header_values(&out);
    let out = mask_url_userinfo(&out);
    let out = mask_query_params(&out);
    mask_prefixed_tokens(&out)
}

/// Issuer-defined credential prefixes. Each entry is (prefix, minimum length of
/// the token body) — the length floor keeps `sk-` out of `task-force`.
const TOKEN_PREFIXES: &[(&str, usize)] = &[
    ("ghp_", 20),
    ("gho_", 20),
    ("ghu_", 20),
    ("ghs_", 20),
    ("ghr_", 20),
    ("github_pat_", 20),
    ("glpat-", 16),
    ("AKIA", 16),
    ("ASIA", 16),
    ("AIza", 30),
    ("sk-ant-", 16),
    ("sk-", 20),
    ("xoxb-", 16),
    ("xoxp-", 16),
    ("xoxa-", 16),
    ("xoxr-", 16),
    ("xoxs-", 16),
    ("npm_", 30),
    ("eyJ", 20), // JWT header segment
];

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || c == '_'
        || c == '-'
        || c == '.'
        || c == '~'
        || c == '+'
        || c == '/'
}

/// True when `pos` starts a word — the previous char cannot be part of a token.
/// Without this, `task-force` contains `sk-` and `mask-` would be masked.
fn at_word_start(text: &str, pos: usize) -> bool {
    text[..pos]
        .chars()
        .next_back()
        .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_')
}

fn mask_prefixed_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'outer: while i < text.len() {
        if !text.is_char_boundary(i) {
            i += 1;
            continue;
        }
        for (prefix, min_body) in TOKEN_PREFIXES {
            if text[i..].starts_with(prefix) && at_word_start(text, i) {
                let body_start = i + prefix.len();
                let body_len: usize = text[body_start..]
                    .chars()
                    .take_while(|c| is_token_char(*c))
                    .map(char::len_utf8)
                    .sum();
                if body_len >= *min_body {
                    out.push_str(REDACTED);
                    i = body_start + body_len;
                    continue 'outer;
                }
            }
        }
        let c = text[i..].chars().next().expect("char boundary");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// A `-----BEGIN … PRIVATE KEY-----` block through its matching `-----END …-----`.
/// Certificates and public keys use the same envelope and are public by design,
/// so only blocks whose type says PRIVATE KEY are masked.
fn mask_pem_blocks(text: &str) -> String {
    const BEGIN: &str = "-----BEGIN ";
    const END: &str = "-----END ";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let Some(start) = rest.find(BEGIN) else { break };
        let Some(end_rel) = rest[start..].find(END) else {
            break;
        };
        let after_end = start + end_rel + END.len();
        let Some(close_rel) = rest[after_end..].find("-----") else {
            break;
        };
        let block_end = after_end + close_rel + "-----".len();
        out.push_str(&rest[..start]);
        if rest[start..block_end].contains("PRIVATE KEY") {
            out.push_str(REDACTED);
        } else {
            out.push_str(&rest[start..block_end]);
        }
        rest = &rest[block_end..];
    }
    out.push_str(rest);
    out
}

/// The value of an `Authorization` (or `Proxy-Authorization`) header, wherever it
/// appears — a header line, a `-H '…'` argument, a JSON string.
fn mask_header_values(text: &str) -> String {
    const HEADERS: &[&str] = &["authorization:", "x-api-key:", "cookie:", "set-cookie:"];
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'outer: while i < text.len() {
        if !text.is_char_boundary(i) {
            i += 1;
            continue;
        }
        for h in HEADERS {
            if lower[i..].starts_with(h) {
                out.push_str(&text[i..i + h.len()]);
                let mut j = i + h.len();
                // Keep the space that follows the colon, then mask the value up to
                // the first delimiter that can end it.
                while j < text.len() && text[j..].starts_with(' ') {
                    out.push(' ');
                    j += 1;
                }
                let end: usize = text[j..]
                    .chars()
                    .take_while(|c| !matches!(c, '\n' | '\r' | '"' | '\'' | ','))
                    .map(char::len_utf8)
                    .sum();
                if end > 0 {
                    out.push_str(REDACTED);
                }
                i = j + end;
                continue 'outer;
            }
        }
        let c = text[i..].chars().next().expect("char boundary");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Secret-bearing query parameters: `?token=…`, `&api_key=…`.
fn mask_query_params(text: &str) -> String {
    const KEYS: &[&str] = &[
        "token=",
        "api_key=",
        "apikey=",
        "access_token=",
        "refresh_token=",
        "secret=",
        "password=",
        "passwd=",
        "signature=",
        "sig=",
        "auth=",
    ];
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'outer: while i < text.len() {
        if !text.is_char_boundary(i) {
            i += 1;
            continue;
        }
        for k in KEYS {
            if lower[i..].starts_with(k) && at_param_start(text, i) {
                out.push_str(&text[i..i + k.len()]);
                let j = i + k.len();
                let end: usize = text[j..]
                    .chars()
                    .take_while(|c| !matches!(c, '&' | '\n' | '\r' | ' ' | '"' | '\'' | '#'))
                    .map(char::len_utf8)
                    .sum();
                if end > 0 {
                    out.push_str(REDACTED);
                }
                i = j + end;
                continue 'outer;
            }
        }
        let c = text[i..].chars().next().expect("char boundary");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// A parameter starts after `?`, `&`, or at the start — so `not_a_token=` and
/// `my_secret=` (a variable assignment) are left alone.
fn at_param_start(text: &str, pos: usize) -> bool {
    matches!(
        text[..pos].chars().next_back(),
        None | Some('?') | Some('&')
    )
}

/// The password half of `scheme://user:password@host`.
fn mask_url_userinfo(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("://") {
        let (head, after) = rest.split_at(pos + 3);
        out.push_str(head);
        // Authority runs to the first '/', '?', '#', or whitespace.
        let auth_len: usize = after
            .chars()
            .take_while(|c| !matches!(c, '/' | '?' | '#' | ' ' | '"' | '\'' | '\n'))
            .map(char::len_utf8)
            .sum();
        let (authority, tail) = after.split_at(auth_len);
        match authority.rsplit_once('@') {
            Some((userinfo, host)) => match userinfo.split_once(':') {
                Some((user, _password)) => {
                    out.push_str(user);
                    out.push(':');
                    out.push_str(REDACTED);
                    out.push('@');
                    out.push_str(host);
                }
                None => out.push_str(authority),
            },
            None => out.push_str(authority),
        }
        rest = tail;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn masked(input: &str) -> String {
        mask_secrets(input)
    }

    #[test]
    fn issuer_prefixed_tokens_are_masked_in_a_command() {
        let out =
            masked("curl -H 'X-Custom: v' https://api.example -d ghp_abcdefghij0123456789ABCD");
        assert!(!out.contains("ghp_abcdefghij"), "{out}");
        assert!(
            out.contains("curl"),
            "the command must stay readable: {out}"
        );
    }

    #[test]
    fn authorization_headers_are_masked_but_the_command_survives() {
        let out =
            masked("curl -H 'Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.abc.def' https://x/y");
        assert!(!out.contains("eyJhbGciOiJIUzI1NiJ9"), "{out}");
        assert!(out.contains("Authorization:"), "{out}");
        assert!(out.contains("https://x/y"), "the URL must survive: {out}");
    }

    #[test]
    fn query_parameter_secrets_are_masked() {
        for (input, leaked) in [
            ("https://x/y?api_key=abc123def456", "abc123def456"),
            ("https://x/y?a=1&token=zzzz9999", "zzzz9999"),
            ("https://x/y?password=hunter2#frag", "hunter2"),
        ] {
            let out = masked(input);
            assert!(!out.contains(leaked), "{input} -> {out}");
            assert!(out.contains("https://x/y"), "{input} -> {out}");
        }
    }

    #[test]
    fn a_password_in_a_url_is_masked_and_the_user_is_kept() {
        let out = masked("git clone https://alice:s3cr3t-pw@git.example/repo.git");
        assert!(!out.contains("s3cr3t-pw"), "{out}");
        assert!(
            out.contains("alice"),
            "the principal is audit-relevant: {out}"
        );
        assert!(out.contains("git.example"), "{out}");
    }

    #[test]
    fn private_key_blocks_are_masked_whole() {
        let pem = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAA\nmore\n-----END OPENSSH PRIVATE KEY-----";
        let out = masked(&format!("echo '{pem}' > /tmp/k"));
        assert!(!out.contains("b3BlbnNzaC1rZXktdjEAAAA"), "{out}");
    }

    #[test]
    fn aws_and_google_key_shapes_are_masked() {
        let out = masked("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE gcloud --key AIzaSyA1234567890abcdefghijklmnopqrstuv");
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"), "{out}");
        assert!(
            !out.contains("AIzaSyA1234567890abcdefghijklmnopqrstuv"),
            "{out}"
        );
    }

    /// The other half of the contract: a redactor that guesses is a redactor
    /// people learn to ignore. Ordinary text must survive untouched.
    #[test]
    fn ordinary_values_are_left_alone() {
        for input in [
            "cargo test --workspace --offline",
            "git commit -m 'task-force: mask-the-thing'",
            "https://docs.example/guide?page=2&sort=asc",
            "rm -rf /tmp/build",
            "echo sk-1", // below the length floor
            "SELECT * FROM users WHERE id = 7",
            "my_secret=not-a-query-param", // not at a parameter boundary
        ] {
            assert_eq!(masked(input), input, "must not be altered: {input}");
        }
    }

    #[test]
    fn masking_is_idempotent() {
        let once = masked("curl -H 'Authorization: Bearer ghp_abcdefghij0123456789ABCD' https://x");
        assert_eq!(masked(&once), once);
    }
}
