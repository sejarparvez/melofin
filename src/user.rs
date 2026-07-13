use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::path::Path;

/// User profile parsed from YouTube Music after login.
/// Cached on disk so the popover can show identity instantly on startup.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UserProfile {
    pub name: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

impl UserProfile {
    pub fn guest() -> Self {
        Self {
            name: "Guest".to_string(),
            ..Default::default()
        }
    }

    pub fn initial(&self) -> String {
        self.name
            .chars()
            .next()
            .unwrap_or('G')
            .to_uppercase()
            .to_string()
    }

    // -- Network ----------------------------------------------------------------

    /// Fetches user profile from YouTube Music using stored cookies.
    /// Blocking — call from a background thread.
    ///
    /// Strategy: fetch the HTML page to extract `INNERTUBE_API_KEY` from
    /// the embedded `ytcfg`, then call the innertube browse API which
    /// returns structured JSON with user profile data.
    pub fn fetch_from_cookies(cookies_path: &Path) -> Self {
        let Ok(contents) = std::fs::read_to_string(cookies_path) else {
            tracing::debug!("no cookies file for profile fetch");
            return Self::guest();
        };

        let cookie_header = build_cookie_header(&contents);
        if cookie_header.is_empty() {
            tracing::debug!("empty cookie header, skipping profile fetch");
            return Self::guest();
        }
        // Debug: log cookie names being sent
        let cookie_names: Vec<&str> = cookie_header
            .split(';')
            .map(|c| c.trim().split('=').next().unwrap_or(""))
            .collect();
        tracing::info!(cookie_count = cookie_names.len(), cookie_names = ?cookie_names, "cookies being sent");

        let ua = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

        // Step 1: fetch the page HTML to get the innertube API key.
        let html = match ureq::get("https://music.youtube.com")
            .set("Cookie", &cookie_header)
            .set("User-Agent", ua)
            .timeout(std::time::Duration::from_secs(10))
            .call()
        {
            Ok(r) => match r.into_string() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("couldn't read YT Music HTML: {e}");
                    return Self::guest();
                }
            },
            Err(e) => {
                tracing::warn!("failed to fetch YT Music page: {e}");
                return Self::guest();
            }
        };

        let api_key = extract_innertube_api_key(&html);
        tracing::debug!(api_key = ?api_key, "extracted innertube API key");

        // Step 2: call the innertube browse API for structured data.
        if let Some(key) = &api_key
            && let Some(profile) = fetch_profile_from_api(&cookie_header, ua, key)
        {
            return profile;
        }

        // Step 3: fall back to parsing the HTML directly.
        tracing::debug!("API fetch didn't yield profile, trying HTML patterns");
        parse_profile_from_html(&html)
    }

    // -- Disk cache -------------------------------------------------------------

    pub fn load(data_dir: &Path) -> Option<Self> {
        let data = std::fs::read_to_string(data_dir.join("user_profile.json")).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save(&self, data_dir: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).context("serializing profile")?;
        std::fs::write(data_dir.join("user_profile.json"), json).context("writing profile cache")
    }

    pub fn remove_cache(data_dir: &Path) {
        let _ = std::fs::remove_file(data_dir.join("user_profile.json"));
    }
}

// ---------------------------------------------------------------------------
// Cookie / HTTP helpers
// ---------------------------------------------------------------------------

pub fn build_cookie_header(contents: &str) -> String {
    contents
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() >= 7 {
                Some(format!("{}={}", fields[5], fields[6]))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Extracts the `INNERTUBE_API_KEY` from a `ytcfg.set({...})` block in
/// the page HTML.
pub fn extract_innertube_api_key(html: &str) -> Option<String> {
    // Look for "INNERTUBE_API_KEY" followed by colon then a quoted string value
    let marker = r#""INNERTUBE_API_KEY""#;
    let pos = html.find(marker)?;
    let rest = &html[pos + marker.len()..];
    // skip optional whitespace + colon + optional whitespace
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?;
    let rest = rest.trim_start();
    // The value is a quoted string: "AIzaSy..."
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let key = &rest[..end];
    if !key.is_empty() && key.len() < 80 {
        Some(key.to_string())
    } else {
        None
    }
}

/// Extracts a cookie value by name from a `Cookie:` header string.
pub fn get_cookie_value(header: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    for part in header.split(';') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix(&prefix) {
            return Some(val.to_string());
        }
    }
    None
}

/// Builds a `SAPISIDHASH` authorization header value required by YouTube's
/// innertube API. Format: `SAPISIDHASH <timestamp>_<sha1(...)`
pub fn build_sapisidhash(cookie_header: &str, origin: &str) -> Option<String> {
    let sapisid = get_cookie_value(cookie_header, "SAPISID")?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    let input = format!("{timestamp} {sapisid} {origin}");
    let hash = Sha1::digest(input.as_bytes());
    let hex = format!("{hash:x}");
    Some(format!("SAPISIDHASH {timestamp}_{hex}"))
}

// ---------------------------------------------------------------------------
// Innertube API approach
// ---------------------------------------------------------------------------

/// Calls the YouTube Music innertube browse API and extracts user profile
/// from the JSON response.
fn fetch_profile_from_api(cookie_header: &str, ua: &str, api_key: &str) -> Option<UserProfile> {
    let origin = "https://music.youtube.com";

    // Try the account/account_menu endpoint — the actual endpoint YouTube Music's
    // web app uses to fetch user profile data (name, avatar, email).
    if let Some(profile) = fetch_profile_from_account_menu(cookie_header, ua, api_key, origin) {
        return Some(profile);
    }

    None
}

fn fetch_profile_from_account_menu(
    cookie_header: &str,
    ua: &str,
    api_key: &str,
    origin: &str,
) -> Option<UserProfile> {
    let url = format!(
        "https://music.youtube.com/youtubei/v1/account/account_menu?key={api_key}&prettyPrint=false"
    );

    let body = serde_json::json!({
        "context": {
            "client": {
                "clientName": "WEB_REMIX",
                "clientVersion": "1.20250710.01.00",
                "hl": "en",
                "gl": "US"
            }
        }
    });

    let body_str = body.to_string();

    let mut req = ureq::post(&url)
        .set("Cookie", cookie_header)
        .set("User-Agent", ua)
        .set("Content-Type", "application/json")
        .set("X-Origin", origin)
        .set("Referer", "https://music.youtube.com/")
        .set("X-Goog-Api-Format-Version", "1")
        .set("X-YouTube-Client-Name", "67")
        .set("X-YouTube-Client-Version", "1.20250710.01.00")
        .timeout(std::time::Duration::from_secs(10));

    if let Some(auth) = build_sapisidhash(cookie_header, origin) {
        req = req.set("Authorization", &auth);
    }

    let response = match req.send_string(&body_str) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("account_menu API failed: {e}");
            return None;
        }
    };

    let text = match response.into_string() {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("couldn't read account_menu response: {e}");
            return None;
        }
    };

    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("couldn't parse account_menu response as JSON: {e}");
            return None;
        }
    };

    extract_profile_from_account_menu_json(&json)
}

/// Parses the account_menu response. Profile lives at:
/// actions[0].openPopupAction.popup.multiPageMenuRenderer.header.activeAccountHeaderRenderer
fn extract_profile_from_account_menu_json(json: &serde_json::Value) -> Option<UserProfile> {
    let header = json
        .get("actions")?
        .as_array()?
        .first()?
        .get("openPopupAction")?
        .get("popup")?
        .get("multiPageMenuRenderer")?
        .get("header")?
        .get("activeAccountHeaderRenderer")?;

    let name = header
        .get("accountName")
        .and_then(|n| n.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first())
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())?;

    let avatar_url = header
        .get("accountPhoto")
        .and_then(|p| p.get("thumbnails"))
        .and_then(|t| t.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|u| u.as_str())
        .map(|s| s.to_string());

    let email = header
        .get("email")
        .and_then(|e| e.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first())
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .filter(|s| s.contains('@'));

    tracing::info!(name = %name, avatar = ?avatar_url, email = ?email, "fetched user profile from account_menu");

    Some(UserProfile {
        name,
        avatar_url,
        email,
    })
}

// ---------------------------------------------------------------------------
// HTML fallback parsing
// ---------------------------------------------------------------------------

fn parse_profile_from_html(html: &str) -> UserProfile {
    let name = extract_name(html).unwrap_or_else(|| "YouTube Music".to_string());
    let avatar_url = extract_avatar_url(html);
    let email = extract_email(html);

    UserProfile {
        name,
        avatar_url,
        email,
    }
}

fn extract_name(html: &str) -> Option<String> {
    let markers = [
        (r#""c4TabbedHeaderRenderer""#, r#""title":{"simpleText":""#),
        (r#""accountRenderer""#, r#""name":{"simpleText":""#),
        (r#""channelMetadataRenderer""#, r#""title":""#),
    ];

    for (context, marker) in &markers {
        let ctx_pos = html.find(context)?;
        let search_area = &html[ctx_pos..];
        if let Some(rel) = search_area.find(marker) {
            let start = ctx_pos + rel + marker.len();
            let end = html[start..].find('"')?;
            let name = &html[start..start + end];
            if !name.is_empty() && name.len() < 80 {
                return Some(name.to_string());
            }
        }
    }

    None
}

fn extract_avatar_url(html: &str) -> Option<String> {
    let marker = "yt3.ggpht.com";
    let mut search_from = 0;
    while let Some(pos) = html[search_from..].find(marker) {
        let abs = search_from + pos;
        let start = html[..abs]
            .rfind(|c: char| ['"', '\'', '('].contains(&c))
            .map(|p| p + 1)?;
        let rest = &html[start..];
        let end = rest
            .find(|c: char| ['"', '\'', ')', ','].contains(&c))
            .unwrap_or(rest.len());
        let url = &rest[..end];
        if url.starts_with("http") && url.len() < 300 {
            return Some(url.to_string());
        }
        search_from = abs + marker.len();
    }
    None
}

fn extract_email(html: &str) -> Option<String> {
    let markers = [
        r#""email":{"simpleText":""#,
        r#""loginEmail":{"simpleText":""#,
    ];
    for marker in &markers {
        if let Some(pos) = html.find(marker) {
            let start = pos + marker.len();
            if let Some(end) = html[start..].find('"') {
                let value = &html[start..start + end];
                if value.contains('@') {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_has_initial() {
        assert_eq!(UserProfile::guest().initial(), "G");
    }

    #[test]
    fn initial_uppercases() {
        let p = UserProfile {
            name: "alice".into(),
            ..Default::default()
        };
        assert_eq!(p.initial(), "A");
    }

    #[test]
    fn build_cookie_header_parses_netscape() {
        let input = "\
# Netscape HTTP Cookie File
.youtube.com\tTRUE\t/\tTRUE\t1818466539\tLOGIN_INFO\tabc123
.youtube.com\tTRUE\t/\tTRUE\t1818466539\tSAPISID\tdef456
";
        assert_eq!(
            build_cookie_header(input),
            "LOGIN_INFO=abc123; SAPISID=def456"
        );
    }

    #[test]
    fn build_cookie_header_skips_blank_and_comment_lines() {
        let input = "# comment\n\n.youtube.com\tTRUE\t/\tTRUE\t0\tX\ty\n";
        assert_eq!(build_cookie_header(input), "X=y");
    }

    #[test]
    fn extract_innertube_api_key_works() {
        let html = r#"var ytcfg = {"INNERTUBE_API_KEY":"AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30","INNERTUBE_CONTEXT":{}};"#;
        assert_eq!(
            extract_innertube_api_key(html).as_deref(),
            Some("AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30")
        );
    }

    #[test]
    fn extract_innertube_api_key_returns_none_when_missing() {
        assert!(extract_innertube_api_key("no api key here").is_none());
    }

    #[test]
    fn extract_profile_from_account_menu_works() {
        let json = serde_json::json!({
            "actions": [{
                "openPopupAction": {
                    "popup": {
                        "multiPageMenuRenderer": {
                            "header": {
                                "activeAccountHeaderRenderer": {
                                    "accountName": {"runs": [{"text": "John Doe"}]},
                                    "accountPhoto": {
                                        "thumbnails": [{"url": "https://yt3.ggpht.com/photo.jpg"}]
                                    },
                                    "email": {"runs": [{"text": "john@gmail.com"}]}
                                }
                            }
                        }
                    }
                }
            }]
        });
        let p = extract_profile_from_account_menu_json(&json).unwrap();
        assert_eq!(p.name, "John Doe");
        assert_eq!(
            p.avatar_url.as_deref(),
            Some("https://yt3.ggpht.com/photo.jpg")
        );
        assert_eq!(p.email.as_deref(), Some("john@gmail.com"));
    }

    #[test]
    fn extract_profile_from_account_menu_no_email() {
        let json = serde_json::json!({
            "actions": [{
                "openPopupAction": {
                    "popup": {
                        "multiPageMenuRenderer": {
                            "header": {
                                "activeAccountHeaderRenderer": {
                                    "accountName": {"runs": [{"text": "Jane Smith"}]},
                                    "accountPhoto": {
                                        "thumbnails": [{"url": "https://yt3.ggpht.com/avatar.jpg"}]
                                    }
                                }
                            }
                        }
                    }
                }
            }]
        });
        let p = extract_profile_from_account_menu_json(&json).unwrap();
        assert_eq!(p.name, "Jane Smith");
        assert_eq!(
            p.avatar_url.as_deref(),
            Some("https://yt3.ggpht.com/avatar.jpg")
        );
        assert!(p.email.is_none());
    }

    #[test]
    fn extract_profile_from_account_menu_returns_none_for_empty() {
        let json = serde_json::json!({});
        assert!(extract_profile_from_account_menu_json(&json).is_none());
    }

    #[test]
    fn parse_profile_falls_back_gracefully() {
        let p = parse_profile_from_html("nothing here");
        assert_eq!(p.name, "YouTube Music");
        assert!(p.avatar_url.is_none());
    }

    #[test]
    fn extract_name_skips_unrelated_json() {
        let html = r#"{"video":{"title":{"simpleText":"Some Song"}},"c4TabbedHeaderRenderer":{"title":{"simpleText":"Real Name"}}"#;
        assert_eq!(extract_name(html).as_deref(), Some("Real Name"));
    }

    #[test]
    fn extract_avatar_returns_first_valid_url() {
        let html = r#"..."url":"https://yt3.ggpht.com/a.jpg","w":48..."#;
        assert_eq!(
            extract_avatar_url(html).as_deref(),
            Some("https://yt3.ggpht.com/a.jpg")
        );
    }

    #[test]
    fn extract_email_works() {
        let html = r#""email":{"simpleText":"user@example.com"}"#;
        assert_eq!(extract_email(html).as_deref(), Some("user@example.com"));
    }
}
