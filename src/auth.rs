//! Cookie-based YouTube Music login.
//!
//! melofin authenticates by reusing a browser session's cookies rather than
//! implementing YouTube's login flow itself — there is no public,
//! TOS-friendly OAuth path for third-party clients, and every YT Music
//! client (this one included, via `yt-dlp`) works around that the same way.
//!
//! The user exports `cookies.txt` from a logged-in browser session (e.g. via
//! the "Get cookies.txt LOCALLY" extension) and melofin hands that file
//! straight to `yt-dlp --cookies`, which both `search.rs` and mpv's
//! `ytdl_hook` already shell out to. That means one imported file covers
//! both search and playback — no separate auth client, no embedded browser.
//!
//! This module owns the cookies file on disk and knows how to import,
//! validate, and remove it. It has no GTK dependency on purpose, so the
//! login dialog (`ui/login_dialog.rs`) can stay a thin wrapper that just
//! calls into this and renders the result.

use anyhow::{Context, Result, bail};
use rookie::common::enums::Cookie as RookieCookie;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState {
    LoggedOut,
    LoggedIn,
}

/// Owns melofin's on-disk cookies file. `data_dir` is melofin's XDG data
/// directory (e.g. `glib::user_data_dir().join("melofin")`); the caller is
/// responsible for creating it before constructing this.
#[derive(Clone)]
pub struct AuthManager {
    cookies_path: PathBuf,
}

impl AuthManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            cookies_path: data_dir.join("cookies.txt"),
        }
    }

    /// Path to hand to `yt-dlp --cookies` / mpv's ytdl_hook once logged in.
    /// Callers should only use this when `current_state()` is `LoggedIn`.
    pub fn cookies_path(&self) -> &Path {
        &self.cookies_path
    }

    /// Cheap "probably logged in" check for things like startup UI state —
    /// it only checks the file exists, it does NOT confirm the session is
    /// still valid (YouTube sessions can expire independently of the file
    /// being there). Call `validate()` when you need to be sure: right
    /// after import, or when a search/playback call fails unexpectedly and
    /// you suspect the cookies expired.
    pub fn current_state(&self) -> AuthState {
        if self.cookies_path.exists() {
            AuthState::LoggedIn
        } else {
            AuthState::LoggedOut
        }
    }

    /// Copies `source` (the file the user picked in a file chooser) into
    /// melofin's data dir, then validates it actually works. On validation
    /// failure the copied file is removed again, so a bad/expired import
    /// can't leave a stale `AuthState::LoggedIn` behind.
    pub async fn import_cookies_file(&self, source: &Path) -> Result<()> {
        let contents = tokio::fs::read(source)
            .await
            .with_context(|| format!("couldn't read {}", source.display()))?;

        if let Some(parent) = self.cookies_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::write(&self.cookies_path, &contents)
            .await
            .context("couldn't save cookies file")?;

        // Cookies file holds a live session credential — lock it down like
        // one. Mirrors the 0700 dir permissions set up in Phase 0.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = tokio::fs::metadata(&self.cookies_path).await {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = tokio::fs::set_permissions(&self.cookies_path, perms).await;
            }
        }

        if let Err(e) = self.validate().await {
            let _ = tokio::fs::remove_file(&self.cookies_path).await;
            return Err(e);
        }

        Ok(())
    }

    /// Confirms the stored cookies represent a real signed-in session by
    /// checking for cookies that only exist after logging in — `LOGIN_INFO`
    /// and `SAPISID`/`__Secure-3PAPISID` are set at login and absent from
    /// an anonymous session's cookies.txt (which only has things like
    /// `YSC`/`PREF`/`VISITOR_INFO1_LIVE`).
    ///
    /// This checks the file directly rather than probing YouTube over the
    /// network. A live probe (hitting the `LM`/`WL` auto-playlists, or
    /// `/library`, `/subscriptions`, `/history`) was the original approach,
    /// but every one of those goes through yt-dlp's `youtube:tab`
    /// extractor, which — as of yt-dlp 2026.07.04 — returns "Incomplete yt
    /// initial data" / 404s on all of them regardless of auth state
    /// (confirmed by hand: an age-restricted video also failed identically
    /// with and without valid cookies, ruling out an auth-detection
    /// problem specific to those URLs). Checking the cookie file's own
    /// contents sidesteps that extractor entirely, is instant (no
    /// subprocess/network round trip), and isn't at the mercy of which
    /// YouTube endpoint yt-dlp currently supports.
    pub async fn validate(&self) -> Result<()> {
        if !self.cookies_path.exists() {
            bail!("no cookies file to validate");
        }

        let contents = tokio::fs::read_to_string(&self.cookies_path)
            .await
            .context("couldn't read cookies file")?;

        if !cookies_look_signed_in(&contents) {
            bail!(
                "cookies file doesn't look like a signed-in session — \
                 missing LOGIN_INFO/SAPISID. Make sure you exported \
                 cookies while actually logged into music.youtube.com."
            );
        }

        Ok(())
    }

    /// Removes the stored cookies file. Idempotent — logging out when
    /// already logged out is not an error, matching how `search.rs` treats
    /// missing-but-expected states elsewhere in this codebase.
    pub async fn logout(&self) -> Result<()> {
        match tokio::fs::remove_file(&self.cookies_path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).context("couldn't remove cookies file"),
        }
    }

    /// Imports cookies read by `rookie` from the user's browser, converts
    /// them to Netscape format, writes the file, and validates. Rolls back
    /// on validation failure, same as `import_cookies_file`.
    pub async fn import_cookies_from_rookie(&self, cookies: Vec<RookieCookie>) -> Result<()> {
        if cookies.is_empty() {
            bail!("no cookies returned from browser — are you logged into YouTube Music?");
        }

        let contents = cookies_to_netscape(&cookies);

        if let Some(parent) = self.cookies_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::write(&self.cookies_path, &contents)
            .await
            .context("couldn't save cookies file")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = tokio::fs::metadata(&self.cookies_path).await {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = tokio::fs::set_permissions(&self.cookies_path, perms).await;
            }
        }

        if let Err(e) = self.validate().await {
            let _ = tokio::fs::remove_file(&self.cookies_path).await;
            return Err(e);
        }

        Ok(())
    }
}

/// Converts a list of `rookie::Cookie`s to Netscape cookie-file format.
pub fn cookies_to_netscape(cookies: &[RookieCookie]) -> String {
    let mut out = String::from("# Netscape HTTP Cookie File\n");
    for c in cookies {
        let include_subdomains = if c.domain.starts_with('.') {
            "TRUE"
        } else {
            "FALSE"
        };
        let secure = if c.secure { "TRUE" } else { "FALSE" };
        let expires = c.expires.unwrap_or(0);
        out += &format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            c.domain, include_subdomains, c.path, secure, expires, c.name, c.value,
        );
    }
    out
}

/// Known browser config directories on Linux. Each entry is
/// `(display_name, relative_config_path)` under `$HOME`.
pub const KNOWN_BROWSERS: &[(&str, &str)] = &[
    ("Firefox", ".mozilla/firefox"),
    ("Chrome", ".config/google-chrome"),
    ("Chromium", ".config/chromium"),
    ("Brave", ".config/BraveSoftware/Brave-Browser"),
    ("Vivaldi", ".config/vivaldi"),
    ("LibreWolf", ".librewolf"),
];

/// Returns the display names of installed browsers (those whose config
/// directory exists under `$HOME`). Used by the login dialog to show
/// auto-import buttons.
pub fn detect_browsers() -> Vec<&'static str> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let Some(home) = home else {
        return Vec::new();
    };
    KNOWN_BROWSERS
        .iter()
        .filter(|(_, path)| home.join(path).is_dir())
        .map(|(name, _)| *name)
        .collect()
}

/// Calls the appropriate `rookie` function for the given browser name and
/// filters cookies to YouTube Music domains.
pub fn rookie_import(browser: &str) -> Result<Vec<RookieCookie>> {
    let domains = vec![
        "youtube.com".into(),
        "music.youtube.com".into(),
        ".youtube.com".into(),
    ];
    let result = match browser {
        "Firefox" => rookie::firefox(Some(domains)),
        "Chrome" => rookie::chrome(Some(domains)),
        "Chromium" => rookie::chromium(Some(domains)),
        "Brave" => rookie::brave(Some(domains)),
        "Vivaldi" => rookie::vivaldi(Some(domains)),
        "LibreWolf" => rookie::librewolf(Some(domains)),
        _ => bail!("unsupported browser: {browser}"),
    };
    result.map_err(|e| anyhow::anyhow!("{e}"))
}

/// Parses a Netscape-format cookies.txt (tab-separated: domain,
/// include-subdomains, path, secure, expiry, name, value) and checks
/// whether the login-only cookie names are present. Pulled out as a
/// free function, separate from any file/tokio access, so it's trivially
/// unit-testable with an in-memory string.
fn cookies_look_signed_in(contents: &str) -> bool {
    let cookie_names: HashSet<&str> = contents
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .filter_map(|line| line.split('\t').nth(5))
        .collect();

    let has_login_info = cookie_names.contains("LOGIN_INFO");
    let has_sapisid =
        cookie_names.contains("SAPISID") || cookie_names.contains("__Secure-3PAPISID");

    has_login_info && has_sapisid
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal temp-dir helper so this module doesn't need a `tempfile` dev
    /// dependency just for one test — matches this project's preference for
    /// not adding deps for small conveniences (see search.rs's approach to
    /// keeping parsing dependency-free).
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "melofin-test-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const SIGNED_IN_COOKIES: &str = "\
# Netscape HTTP Cookie File
.youtube.com\tTRUE\t/\tTRUE\t1818466539\tLOGIN_INFO\tabc123
.youtube.com\tTRUE\t/\tTRUE\t1818466539\tSAPISID\tdef456
.youtube.com\tTRUE\t/\tFALSE\t1783565701\tYSC\tghi789
";

    const ANONYMOUS_COOKIES: &str = "\
# Netscape HTTP Cookie File
.youtube.com\tTRUE\t/\tFALSE\t1783565701\tYSC\tghi789
.youtube.com\tTRUE\t/\tTRUE\t1799958767\tVISITOR_INFO1_LIVE\txyz000
.youtube.com\tTRUE\t/\tTRUE\t1818125700\tPREF\ttz=1
";

    #[test]
    fn reports_logged_out_when_no_cookies_file_present() {
        let dir = TempDir::new("logged-out");
        let mgr = AuthManager::new(dir.path());
        assert_eq!(mgr.current_state(), AuthState::LoggedOut);
    }

    #[test]
    fn reports_logged_in_when_cookies_file_present() {
        let dir = TempDir::new("logged-in");
        let mgr = AuthManager::new(dir.path());
        std::fs::write(mgr.cookies_path(), SIGNED_IN_COOKIES).unwrap();
        assert_eq!(mgr.current_state(), AuthState::LoggedIn);
    }

    #[test]
    fn recognizes_signed_in_cookies() {
        assert!(cookies_look_signed_in(SIGNED_IN_COOKIES));
    }

    #[test]
    fn recognizes_secure_prefixed_sapisid_variant() {
        let contents = "\
# Netscape HTTP Cookie File
.youtube.com\tTRUE\t/\tTRUE\t1818466539\tLOGIN_INFO\tabc123
.youtube.com\tTRUE\t/\tTRUE\t1818466539\t__Secure-3PAPISID\tdef456
";
        assert!(cookies_look_signed_in(contents));
    }

    #[test]
    fn rejects_anonymous_cookies() {
        assert!(!cookies_look_signed_in(ANONYMOUS_COOKIES));
    }

    #[test]
    fn rejects_empty_file() {
        assert!(!cookies_look_signed_in(""));
    }

    #[tokio::test]
    async fn validate_fails_when_no_file_exists() {
        let dir = TempDir::new("validate-missing");
        let mgr = AuthManager::new(dir.path());
        let result = mgr.validate().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validate_fails_on_anonymous_cookies() {
        let dir = TempDir::new("validate-anonymous");
        let mgr = AuthManager::new(dir.path());
        std::fs::write(mgr.cookies_path(), ANONYMOUS_COOKIES).unwrap();
        let result = mgr.validate().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn validate_succeeds_on_signed_in_cookies() {
        let dir = TempDir::new("validate-signed-in");
        let mgr = AuthManager::new(dir.path());
        std::fs::write(mgr.cookies_path(), SIGNED_IN_COOKIES).unwrap();
        mgr.validate()
            .await
            .expect("should validate signed-in cookies");
    }

    #[tokio::test]
    async fn logout_is_idempotent_when_already_logged_out() {
        let dir = TempDir::new("logout-idempotent");
        let mgr = AuthManager::new(dir.path());
        assert_eq!(mgr.current_state(), AuthState::LoggedOut);
        mgr.logout()
            .await
            .expect("logout on empty state should not error");
    }

    #[tokio::test]
    async fn logout_removes_cookies_file() {
        let dir = TempDir::new("logout-removes-file");
        let mgr = AuthManager::new(dir.path());
        std::fs::write(mgr.cookies_path(), SIGNED_IN_COOKIES).unwrap();
        assert_eq!(mgr.current_state(), AuthState::LoggedIn);

        mgr.logout().await.unwrap();
        assert_eq!(mgr.current_state(), AuthState::LoggedOut);
    }

    #[tokio::test]
    async fn import_rejects_anonymous_cookies_and_rolls_back() {
        let source_dir = TempDir::new("import-source-anon");
        let data_dir = TempDir::new("import-data-anon");
        let source_path = source_dir.path().join("cookies.txt");
        std::fs::write(&source_path, ANONYMOUS_COOKIES).unwrap();

        let mgr = AuthManager::new(data_dir.path());
        let result = mgr.import_cookies_file(&source_path).await;

        assert!(result.is_err());
        assert_eq!(mgr.current_state(), AuthState::LoggedOut);
        assert!(!mgr.cookies_path().exists());
    }

    #[tokio::test]
    async fn import_accepts_signed_in_cookies() {
        let source_dir = TempDir::new("import-source-ok");
        let data_dir = TempDir::new("import-data-ok");
        let source_path = source_dir.path().join("cookies.txt");
        std::fs::write(&source_path, SIGNED_IN_COOKIES).unwrap();

        let mgr = AuthManager::new(data_dir.path());
        mgr.import_cookies_file(&source_path)
            .await
            .expect("should accept signed-in cookies");

        assert_eq!(mgr.current_state(), AuthState::LoggedIn);
    }
}
