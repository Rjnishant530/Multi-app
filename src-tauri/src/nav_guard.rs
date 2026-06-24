#![allow(dead_code)] // wired into webview navigation handlers in U5.
//! Same-site classification for navigation interception.
//!
//! The product behavior is "stay in the current instance for same-site
//! navigation; fork to a new child instance for cross-site." The unit of
//! site identity is the **registered domain** (eTLD+1) computed from the
//! Public Suffix List — not the exact host. This keeps SSO redirects
//! through different subdomains (e.g. accounts.google.com → mail.google.com)
//! inside one instance.

use url::Url;

/// Returns the registered domain (eTLD+1) for the host of `url`, or
/// `None` if the URL has no host or no recognizable public suffix
/// (file://, about:, malformed, etc.).
pub fn registered_domain(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    psl::domain_str(host).map(|s| s.to_string())
}

/// `true` when `candidate` is reachable from a webview pinned to
/// `parent_root` without forking. Returns `false` for any URL whose
/// registered domain cannot be determined — same-site is an
/// allow-listing decision, so the safe default is "treat as cross-site."
pub fn is_same_site(parent_root: &str, candidate: &Url) -> bool {
    let Some(candidate_root) = registered_domain(candidate) else {
        return false;
    };
    parent_root.eq_ignore_ascii_case(&candidate_root)
}

/// `true` when navigating to `candidate` from a webview pinned to
/// `parent_root` should spawn a new child instance.
///
/// Distinct from `!is_same_site`: URLs without a registered domain
/// (about:blank, data:, blob:, file:, IP literals, localhost) are NOT
/// forked — they're transient, scheme-shaped, or dev-only, and trying
/// to spawn a fresh session-isolated webview at about:blank just errors
/// out. The user-observable rule: forks happen between distinct REAL
/// public sites.
pub fn should_fork(parent_root: &str, candidate: &Url) -> bool {
    use url::Host;
    match candidate.host() {
        Some(Host::Domain(_)) => match registered_domain(candidate) {
            None => false,
            Some(root) => !parent_root.eq_ignore_ascii_case(&root),
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).expect("test URL must parse")
    }

    #[test]
    fn google_subdomains_are_same_site() {
        assert!(is_same_site("google.com", &u("https://mail.google.com/inbox")));
        assert!(is_same_site(
            "google.com",
            &u("https://accounts.google.com/o/oauth2/v2/auth"),
        ));
        assert!(is_same_site("google.com", &u("https://docs.google.com/")));
        assert!(is_same_site("google.com", &u("https://www.google.com")));
    }

    #[test]
    fn different_etld_plus_one_is_cross_site() {
        assert!(!is_same_site("google.com", &u("https://google.co.uk")));
        assert!(!is_same_site("google.com", &u("https://stripe.com/pricing")));
        assert!(!is_same_site("github.com", &u("https://github.io")));
        assert!(!is_same_site("github.com", &u("https://gitlab.com")));
    }

    #[test]
    fn psl_handles_multi_level_suffixes_correctly() {
        // a.b.co.uk -> b.co.uk is the registered domain; sub.b.co.uk is same site as b.co.uk
        assert_eq!(
            registered_domain(&u("https://sub.example.co.uk")).as_deref(),
            Some("example.co.uk"),
        );
        assert!(is_same_site(
            "example.co.uk",
            &u("https://sub.example.co.uk/path"),
        ));
        // Different registered domain entirely
        assert!(!is_same_site(
            "example.co.uk",
            &u("https://other.co.uk"),
        ));
    }

    #[test]
    fn comparison_is_case_insensitive_on_the_root() {
        assert!(is_same_site("Google.COM", &u("https://mail.google.com")));
    }

    #[test]
    fn urls_without_a_host_are_cross_site() {
        assert!(!is_same_site("google.com", &u("about:blank")));
        assert!(!is_same_site(
            "google.com",
            &u("data:text/html,<h1>hi</h1>"),
        ));
    }

    #[test]
    fn registered_domain_returns_none_for_hostless_url() {
        assert!(registered_domain(&u("file:///tmp/page.html")).is_none());
    }

    #[test]
    fn should_fork_only_when_both_sides_have_a_real_registered_domain() {
        // Same registered domain → no fork.
        assert!(!should_fork("google.com", &u("https://mail.google.com")));
        // Different registered domain → fork.
        assert!(should_fork("google.com", &u("https://stripe.com/checkout")));
        // No registered domain at all → don't fork.
        assert!(!should_fork("google.com", &u("about:blank")));
        assert!(!should_fork("google.com", &u("data:text/html,<h1>hi</h1>")));
        assert!(!should_fork("google.com", &u("file:///tmp/page.html")));
        assert!(!should_fork("google.com", &u("http://127.0.0.1:3000/")));
        assert!(!should_fork("google.com", &u("http://localhost:8080/")));
    }

    #[test]
    fn ip_literals_are_not_same_site_as_any_domain() {
        // psl::domain_str returns None for IP literals.
        assert!(!is_same_site("google.com", &u("http://127.0.0.1/")));
        assert!(!is_same_site("google.com", &u("http://10.0.0.1:8080/")));
    }
}
