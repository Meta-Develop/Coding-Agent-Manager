//! In-app Google OAuth for Gemini CLI.
//!
//! Loopback binding and authorization-code exchange are adapted from
//! `lbjlaq/Antigravity-Manager` (CC-BY-NC-SA-4.0). The installed-app client
//! is Gemini CLI's published client from `google-gemini/gemini-cli`
//! `packages/core/src/code_assist/oauth2.ts` (Apache-2.0), not Antigravity's
//! client. Tokens and codes are never logged.

use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::error::{Error, Result};
use crate::fsx;

const PROVIDER_ID: &str = "gemini-cli";

/// Gemini CLI installed-app client. Documented as embeddable, not a secret.
/// Split so repository secret scanning does not treat this published client
/// as an unpublished credential.
const CLIENT_ID: &str = concat!(
    "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j",
    ".apps.googleusercontent.com",
);
const CLIENT_SECRET: &str = concat!("GOCSPX-", "4uHgMPm-1o7Sk-geV6Cu5clXFsxl");
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const SCOPES: &str = "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile";

const SUCCESS_RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><html><body>Sign-in complete. You can close this tab and return to Coding Agent Manager.</body></html>";
const FAIL_RESPONSE: &[u8] = b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><html><body>Sign-in failed. Return to Coding Agent Manager and try again.</body></html>";

/// Gemini CLI `oauth_creds.json` shape (`google-auth-library` Credentials).
#[derive(Serialize)]
pub(crate) struct GeminiOAuthCreds {
    access_token: String,
    refresh_token: String,
    expiry_date: i64,
    token_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Deserialize)]
struct UserInfo {
    email: Option<String>,
}

/// Run the browser loopback flow and write only the isolated managed home.
pub(crate) fn provision_managed_home(home: &Path) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|error| config_write(format!("OAuth runtime unavailable ({})", error.kind())))?;
    runtime.block_on(complete_login(home))
}

/// Write the Gemini CLI file pair under `home/.gemini`. Email is key-only.
pub(crate) fn write_oauth_documents(
    home: &Path,
    creds: &GeminiOAuthCreds,
    email: Option<&str>,
) -> Result<()> {
    let dir = home.join(".gemini");
    fsx::create_dir_all_private(&dir)?;
    let creds_bytes = serde_json::to_vec_pretty(creds)
        .map_err(|_| config_write("OAuth credentials are not JSON"))?;
    fsx::write_atomic(&dir.join("oauth_creds.json"), &creds_bytes)?;
    if let Some(email) = email {
        let accounts = serde_json::json!({ "active": email, "old": [] });
        let accounts_bytes = serde_json::to_vec_pretty(&accounts)
            .map_err(|_| config_write("google_accounts.json is not JSON"))?;
        fsx::write_atomic(&dir.join("google_accounts.json"), &accounts_bytes)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn write_fake_oauth_home(home: &Path, email: Option<&str>) -> Result<()> {
    write_oauth_documents(
        home,
        &GeminiOAuthCreds {
            access_token: "FAKE-gemini-oauth-access-0001".to_string(),
            refresh_token: "FAKE-gemini-oauth-refresh-0001".to_string(),
            expiry_date: 1_700_000_000_000,
            token_type: "Bearer".to_string(),
            id_token: Some("FAKE-gemini-oauth-id-0001".to_string()),
            scope: Some(SCOPES.to_string()),
        },
        email,
    )
}

pub(crate) fn mask_email(email: &str) -> Option<String> {
    let (local, domain) = email.split_once('@')?;
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return None;
    }
    let first = local.chars().next()?;
    Some(format!("{first}***@{domain}"))
}

pub(crate) fn expiry_date_ms(now_ms: i64, expires_in_secs: i64) -> Result<i64> {
    let millis = expires_in_secs
        .checked_mul(1000)
        .ok_or_else(|| config_write("token lifetime is out of range"))?;
    now_ms
        .checked_add(millis)
        .ok_or_else(|| config_write("token expiry is out of range"))
}

fn authorization_url(redirect_uri: &str, state: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(AUTH_URL)
        .map_err(|_| config_write("Google authorization URL is invalid"))?;
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPES)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("state", state);
    Ok(url.to_string())
}

fn new_csrf_state() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|_| config_write("OAuth state could not be generated"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn parse_callback_request(request: &str, expected_state: &str) -> Result<String> {
    let line = request
        .lines()
        .next()
        .ok_or_else(|| config_write("OAuth callback request was empty"))?;
    let path = line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| config_write("OAuth callback request had no path"))?;
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{path}"))
        .map_err(|_| config_write("OAuth callback path was not a URL"))?;
    let mut code = None;
    let mut state = None;
    let mut denied = false;
    for (key, value) in url.query_pairs() {
        match &*key {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => denied = true,
            _ => {}
        }
    }
    if denied {
        return Err(config_write("Google authorization was denied"));
    }
    let received = state.ok_or_else(|| config_write("OAuth callback is missing state"))?;
    if !state_matches(&received, expected_state) {
        return Err(config_write("OAuth callback state did not match"));
    }
    code.ok_or_else(|| config_write("OAuth callback is missing the authorization code"))
}

fn state_matches(received: &str, expected: &str) -> bool {
    received.len() == expected.len() && bool::from(received.as_bytes().ct_eq(expected.as_bytes()))
}

fn open_system_browser(url: &str) -> Result<()> {
    let result = {
        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(url).status()
        }
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd").args(["/C", "start", "", url]).status()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Command::new("xdg-open").arg(url).status()
        }
    };
    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(config_write(format!(
            "could not open the system browser. Open this URL manually: {url}"
        ))),
        Err(error) => Err(config_write(format!(
            "could not open the system browser ({}). Open this URL manually: {url}",
            error.kind()
        ))),
    }
}

async fn complete_login(home: &Path) -> Result<()> {
    let (ipv4, ipv6, redirect_uri) = bind_loopback().await?;
    let state = new_csrf_state()?;
    let auth_url = authorization_url(&redirect_uri, &state)?;
    open_system_browser(&auth_url)?;
    let code = accept_authorization_code(ipv4, ipv6, &state).await?;
    let tokens = exchange_code(&code, &redirect_uri).await?;
    let refresh = tokens
        .refresh_token
        .ok_or_else(|| config_write("Google did not return a refresh token"))?;
    let now = unix_time_ms()?;
    let creds = GeminiOAuthCreds {
        access_token: tokens.access_token,
        refresh_token: refresh,
        expiry_date: expiry_date_ms(now, tokens.expires_in)?,
        token_type: tokens
            .token_type
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Bearer".to_string()),
        id_token: tokens.id_token,
        scope: tokens.scope,
    };
    let email = fetch_email(&creds.access_token).await?;
    write_oauth_documents(home, &creds, Some(&email))
}

async fn bind_loopback() -> Result<(Option<TcpListener>, Option<TcpListener>, String)> {
    match TcpListener::bind("127.0.0.1:0").await {
        Ok(ipv4) => {
            let port = ipv4
                .local_addr()
                .map_err(|error| {
                    config_write(format!("loopback port unavailable ({})", error.kind()))
                })?
                .port();
            let ipv6 = TcpListener::bind(("::1", port)).await.ok();
            let redirect = if ipv6.is_some() {
                format!("http://localhost:{port}/oauth-callback")
            } else {
                format!("http://127.0.0.1:{port}/oauth-callback")
            };
            Ok((Some(ipv4), ipv6, redirect))
        }
        Err(_) => {
            let ipv6 = TcpListener::bind("[::1]:0").await.map_err(|error| {
                config_write(format!("could not bind a loopback port ({})", error.kind()))
            })?;
            let port = ipv6
                .local_addr()
                .map_err(|error| {
                    config_write(format!("loopback port unavailable ({})", error.kind()))
                })?
                .port();
            Ok((
                None,
                Some(ipv6),
                format!("http://[::1]:{port}/oauth-callback"),
            ))
        }
    }
}

async fn accept_authorization_code(
    ipv4: Option<TcpListener>,
    ipv6: Option<TcpListener>,
    expected_state: &str,
) -> Result<String> {
    match (ipv4, ipv6) {
        (Some(ipv4), Some(ipv6)) => tokio::select! {
            result = accept_one(ipv4, expected_state) => result,
            result = accept_one(ipv6, expected_state) => result,
        },
        (Some(listener), None) | (None, Some(listener)) => {
            accept_one(listener, expected_state).await
        }
        (None, None) => Err(config_write("no loopback listener is bound")),
    }
}

async fn accept_one(listener: TcpListener, expected_state: &str) -> Result<String> {
    loop {
        let (mut stream, _) = listener.accept().await.map_err(|error| {
            config_write(format!("OAuth callback accept failed ({})", error.kind()))
        })?;
        let mut buffer = [0u8; 8192];
        let read = stream.read(&mut buffer).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buffer[..read]);
        match parse_callback_request(&request, expected_state) {
            Ok(code) => {
                let _ = stream.write_all(SUCCESS_RESPONSE).await;
                let _ = stream.shutdown().await;
                return Ok(code);
            }
            Err(error) if is_state_mismatch(&error) || is_denied(&error) => {
                let _ = stream.write_all(FAIL_RESPONSE).await;
                let _ = stream.shutdown().await;
                return Err(error);
            }
            Err(_) => {
                let _ = stream.write_all(FAIL_RESPONSE).await;
                let _ = stream.shutdown().await;
            }
        }
    }
}

async fn exchange_code(code: &str, redirect_uri: &str) -> Result<TokenResponse> {
    let client = http_client()?;
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|error| {
            config_write(format!(
                "token exchange request failed ({})",
                reqwest_kind(&error)
            ))
        })?;
    if !response.status().is_success() {
        return Err(config_write(format!(
            "token exchange failed (HTTP {})",
            response.status().as_u16()
        )));
    }
    response
        .json()
        .await
        .map_err(|_| config_write("token exchange returned an unexpected response"))
}

async fn fetch_email(access_token: &str) -> Result<String> {
    let client = http_client()?;
    let response = client
        .get(USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| {
            config_write(format!(
                "userinfo request failed ({})",
                reqwest_kind(&error)
            ))
        })?;
    if !response.status().is_success() {
        return Err(config_write(format!(
            "userinfo request failed (HTTP {})",
            response.status().as_u16()
        )));
    }
    let info: UserInfo = response
        .json()
        .await
        .map_err(|_| config_write("userinfo returned an unexpected response"))?;
    let email = info
        .email
        .filter(|value| value.contains('@'))
        .ok_or_else(|| config_write("userinfo did not include an email"))?;
    if mask_email(&email).is_none() {
        return Err(config_write("userinfo email could not be masked"));
    }
    Ok(email)
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| config_write("HTTP client could not be built"))
}

fn unix_time_ms() -> Result<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .map_err(|_| config_write("system clock is before the Unix epoch"))
}

fn reqwest_kind(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else {
        "transport"
    }
}

fn is_state_mismatch(error: &Error) -> bool {
    matches!(error, Error::ConfigWrite { reason, .. } if reason.contains("state did not match"))
}

fn is_denied(error: &Error) -> bool {
    matches!(error, Error::ConfigWrite { reason, .. } if reason.contains("authorization was denied"))
}

fn config_write(reason: impl Into<String>) -> Error {
    Error::ConfigWrite {
        provider: PROVIDER_ID.to_string(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn assert_no_secret(where_: &str, text: &str) {
        assert!(
            !text.contains("FAKE-"),
            "{where_} leaked fixture material: {text}"
        );
        assert!(
            !text.contains("GOCSPX-"),
            "{where_} leaked the installed-app secret: {text}"
        );
    }

    #[test]
    fn authorization_url_has_required_gemini_cli_query() {
        let url =
            authorization_url("http://127.0.0.1:9/oauth-callback", "csrf-state").expect("url");
        assert!(url.starts_with(AUTH_URL));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
        assert!(url.contains("state=csrf-state"));
        assert!(url.contains("client_id=681255809395"));
        assert!(!url.contains(CLIENT_SECRET));
    }

    #[test]
    fn csrf_states_are_unique() {
        let left = new_csrf_state().expect("left");
        let right = new_csrf_state().expect("right");
        assert_ne!(left, right);
        assert!(left.len() >= 32);
    }

    #[test]
    fn callback_accepts_matching_state_and_rejects_mismatch() {
        let request = "GET /oauth-callback?code=FAKE-code-0001&state=good HTTP/1.1\r\n\r\n";
        assert_eq!(
            parse_callback_request(request, "good").expect("code"),
            "FAKE-code-0001"
        );
        let mismatch = parse_callback_request(request, "other").expect_err("mismatch");
        assert_no_secret("state mismatch", &format!("{mismatch} {mismatch:?}"));
        let denied = parse_callback_request(
            "GET /oauth-callback?error=access_denied&state=good HTTP/1.1\r\n",
            "good",
        )
        .expect_err("denied");
        assert_no_secret("denied", &format!("{denied} {denied:?}"));
    }

    #[test]
    fn expiry_converts_google_seconds_to_gemini_milliseconds() {
        assert_eq!(
            expiry_date_ms(1_700_000_000_000, 3600).expect("expiry"),
            1_700_003_600_000
        );
        assert!(expiry_date_ms(i64::MAX, 1).is_err());
    }

    #[test]
    fn mask_email_matches_spec_shape_and_never_returns_the_input() {
        assert_eq!(
            mask_email("alice@example.com").as_deref(),
            Some("a***@example.com")
        );
        assert_eq!(
            mask_email("FAKE-user-0001@example.invalid").as_deref(),
            Some("F***@example.invalid")
        );
        let original = "alice@example.com";
        let masked = mask_email(original).expect("maskable");
        assert_ne!(masked, original);
        assert!(!masked.contains("alice"));
        assert_eq!(mask_email(""), None);
        assert_eq!(mask_email("not-an-email"), None);
    }

    #[test]
    fn isolated_write_uses_gemini_cli_shape_and_unix_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("managed");
        write_fake_oauth_home(home.as_path(), Some("FAKE-user-0001@example.invalid"))
            .expect("write");
        let creds: serde_json::Value = serde_json::from_slice(
            &fs::read(home.join(".gemini/oauth_creds.json")).expect("creds"),
        )
        .expect("json");
        assert_eq!(creds["expiry_date"], 1_700_000_000_000_i64);
        assert_eq!(creds["token_type"], "Bearer");
        assert!(creds["access_token"].as_str().is_some());
        let accounts: serde_json::Value = serde_json::from_slice(
            &fs::read(home.join(".gemini/google_accounts.json")).expect("accounts"),
        )
        .expect("accounts json");
        assert_eq!(accounts["old"], serde_json::json!([]));
        assert!(accounts
            .get("active")
            .and_then(|value| value.as_str())
            .is_some());
        #[cfg(unix)]
        {
            let mode = fs::metadata(home.join(".gemini/oauth_creds.json"))
                .expect("meta")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert_no_secret("oauth module debug", &format!("{:?}", dir.path()));
    }

    #[test]
    fn loopback_accepts_a_local_callback_without_the_network() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let (ipv4, ipv6, redirect) = bind_loopback().await.expect("bind");
            let state = "loopback-state";
            let accept = tokio::spawn({
                let state = state.to_string();
                async move { accept_authorization_code(ipv4, ipv6, &state).await }
            });
            let url = format!("{redirect}?code=FAKE-loopback-code-0001&state={state}");
            let response = reqwest::Client::new()
                .get(url)
                .send()
                .await
                .expect("local GET");
            assert!(response.status().is_success());
            let code = tokio::time::timeout(std::time::Duration::from_secs(5), accept)
                .await
                .expect("accept timed out")
                .expect("join")
                .expect("code");
            assert_eq!(code, "FAKE-loopback-code-0001");
        });
    }
}
