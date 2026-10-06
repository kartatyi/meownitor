// Plan limits: the request Claude Code's own /usage makes (GET /api/oauth/usage) — an account query,
// not a model call, so it costs nothing against the limits. It uses Claude Code's sign-in from
// ~/.claude/.credentials.json; when the access token is about to expire it is refreshed the way
// Claude Code refreshes it and written back, so a single `claude` login keeps the widget going.
// The endpoint is rate-limited per account and Claude Code / Desktop query it too, so a 429 is an
// ordinary hiccup: the last numbers stay on the card and the next tries come less often.
// On macOS Claude Code keeps the sign-in in the Keychain instead of the file. There it is only read,
// never refreshed: the refresh token rotates, and a running Claude Code holding the old one would
// lose its sign-in. An expired token waits for Claude Code to refresh it.
// No token ever leaves this module and nothing here is logged.
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// Claude Code's production OAuth client.
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
/// Refresh this long before the token expires.
const EARLY_MS: u64 = 5 * 60 * 1000;
/// Seconds between requests while all is well; a "busy" server doubles it up to `MAX_WAIT`.
const WAIT: u64 = 60;
const MAX_WAIT: u64 = 10 * 60;

/// Why there is no fresh data. Only `Login` means the sign-in itself is unusable.
#[derive(Clone, Copy, PartialEq)]
enum Why {
    Login,
    /// The server answered but asked to wait: 429, or a 5xx.
    Busy,
    Net,
    /// macOS: the Keychain's token has expired and only Claude Code refreshes it there.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Stale,
}

impl Why {
    fn as_str(self) -> &'static str {
        match self {
            Why::Login => "login",
            Why::Busy => "busy",
            Why::Net => "net",
            Why::Stale => "stale",
        }
    }
}

#[derive(Serialize, Clone, Default)]
pub struct Limits {
    /// "ok", or why the last request failed: "login" (no usable Claude Code sign-in), "busy" or "net".
    status: String,
    /// The usage response as the server sent it (percentages and reset times, no secrets). On a
    /// "busy" or "net" failure it is the last good response, so the card keeps its numbers.
    data: Value,
    /// When `data` was received.
    at: u64,
}

pub struct Latest(pub Mutex<Limits>);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn creds_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.map(|h| PathBuf::from(h).join(".claude").join(".credentials.json"))
}

/// The Keychain item Claude Code keeps its sign-in in on macOS, the same JSON as the file.
#[cfg(target_os = "macos")]
fn read_keychain() -> Result<Value, Why> {
    let out = std::process::Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output();
    let r = match out {
        Err(e) => Err(format!("cannot run security: {e}")),
        Ok(o) if !o.status.success() => Err(format!(
            "no Keychain sign-in (security exit {})",
            o.status.code().unwrap_or(-1)
        )),
        Ok(o) => serde_json::from_slice(&o.stdout).map_err(|e| {
            format!(
                "Keychain sign-in is not JSON ({} bytes): {e}",
                o.stdout.len()
            )
        }),
    };
    keychain_note(
        r.as_ref()
            .err()
            .map(String::as_str)
            .unwrap_or("Keychain sign-in read"),
    );
    r.map_err(|_| Why::Login)
}

/// widget.log says why the Keychain gave no sign-in — once per change, never the secret itself.
#[cfg(target_os = "macos")]
fn keychain_note(msg: &str) {
    static LAST: Mutex<String> = Mutex::new(String::new());
    let mut last = LAST.lock().unwrap();
    if *last != msg {
        crate::watchdog::log(&format!("limits: {msg}"));
        *last = msg.to_string();
    }
}

/// The Keychain's access token while it is valid; never refreshed (see the top of the file).
#[cfg(target_os = "macos")]
fn keychain_token(rejected: Option<&str>) -> Result<String, Why> {
    let (access, expires) = access_of(&read_keychain()?);
    if access.is_empty() {
        return Err(Why::Login);
    }
    if expires > now_ms() && rejected != Some(access.as_str()) {
        return Ok(access);
    }
    Err(Why::Stale)
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .build()
}

fn status_why(code: u16) -> Why {
    if code == 429 || code >= 500 {
        Why::Busy
    } else {
        Why::Login
    }
}

/// The credentials file, read again once if it is caught mid-write by Claude Code.
fn read_creds(path: &Path) -> Result<Value, Why> {
    for attempt in 0..2 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(500));
        }
        if let Ok(v) = std::fs::read_to_string(path)
            .map_err(|_| ())
            .and_then(|s| serde_json::from_str(&s).map_err(|_| ()))
        {
            return Ok(v);
        }
    }
    Err(Why::Login)
}

fn access_of(creds: &Value) -> (String, u64) {
    let o = &creds["claudeAiOauth"];
    (
        o["accessToken"].as_str().unwrap_or("").to_string(),
        o["expiresAt"].as_u64().unwrap_or(0),
    )
}

/// A usable access token, refreshing it first when it is (nearly) expired or is the one the
/// server just turned down (`rejected`). A token Claude Code put in the file since then is used as is.
fn token(rejected: Option<&str>) -> Result<String, Why> {
    let path = creds_path().ok_or(Why::Login)?;
    #[cfg(target_os = "macos")]
    if !path.exists() {
        return keychain_token(rejected);
    }
    let mut creds = read_creds(&path)?;
    let (access, expires) = access_of(&creds);
    if !access.is_empty() && expires > now_ms() + EARLY_MS && rejected != Some(access.as_str()) {
        return Ok(access);
    }
    let refresh = creds["claudeAiOauth"]["refreshToken"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if refresh.is_empty() {
        return Err(Why::Login);
    }
    let resp = agent()
        .post(TOKEN_URL)
        .set("Content-Type", "application/json")
        .send_json(json!({ "grant_type": "refresh_token", "refresh_token": refresh, "client_id": CLIENT_ID }));
    let body: Value = match resp {
        Ok(r) => r.into_json().map_err(|_| Why::Net)?,
        Err(ureq::Error::Status(code, _)) => {
            // Refresh tokens rotate: Claude Code may have used this one a moment ago and written
            // the new pair to the file. Then the file has a fresh access token — take it.
            let (now_access, now_expires) =
                read_creds(&path).map(|c| access_of(&c)).unwrap_or_default();
            if !now_access.is_empty() && now_access != access && now_expires > now_ms() {
                return Ok(now_access);
            }
            return Err(status_why(code));
        }
        Err(_) => return Err(Why::Net),
    };
    let new_access = body["access_token"].as_str().ok_or(Why::Login)?.to_string();
    let o = creds.get_mut("claudeAiOauth").ok_or(Why::Login)?;
    o["accessToken"] = json!(new_access);
    if let Some(r) = body["refresh_token"].as_str() {
        o["refreshToken"] = json!(r);
    }
    if let Some(s) = body["expires_in"].as_u64() {
        o["expiresAt"] = json!(now_ms() + s * 1000);
    }
    // Written back whole, so Claude Code itself keeps working with the rotated tokens.
    let tmp = path.with_extension("json.widget-tmp");
    if std::fs::write(
        &tmp,
        serde_json::to_string_pretty(&creds).unwrap_or_default(),
    )
    .is_ok()
        && std::fs::rename(&tmp, &path).is_err()
    {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(new_access)
}

fn usage(tok: &str) -> Result<Value, (u16, Why)> {
    let resp = agent()
        .get(USAGE_URL)
        .set("Authorization", &format!("Bearer {tok}"))
        .set("anthropic-beta", "oauth-2025-04-20")
        .set("Content-Type", "application/json")
        .call();
    match resp {
        Ok(r) => r.into_json().map_err(|_| (0, Why::Net)),
        Err(ureq::Error::Status(code, _)) => Err((code, status_why(code))),
        Err(_) => Err((0, Why::Net)),
    }
}

fn fetch() -> Result<Value, Why> {
    let tok = token(None)?;
    match usage(&tok) {
        Ok(v) => Ok(v),
        // The server may have revoked the token early: get another once and retry.
        Err((401, _)) => usage(&token(Some(&tok))?).map_err(|e| e.1),
        Err((_, why)) => Err(why),
    }
}

/// What the card shows between requests and how long to wait for the next one.
struct Poll {
    last: Option<(Value, u64)>,
    wait: u64,
}

impl Poll {
    fn new() -> Self {
        Poll {
            last: None,
            wait: WAIT,
        }
    }

    /// Takes one request's outcome: the limits to show and the seconds until the next request.
    fn step(&mut self, r: Result<Value, Why>, now: u64) -> (Limits, u64) {
        let limits = match r {
            Ok(data) => {
                self.wait = WAIT;
                self.last = Some((data.clone(), now));
                Limits {
                    status: "ok".into(),
                    data,
                    at: now,
                }
            }
            Err(why) => {
                if why == Why::Login {
                    self.last = None;
                }
                self.wait = match why {
                    Why::Busy => (self.wait * 2).min(MAX_WAIT),
                    Why::Net => 30,
                    Why::Login | Why::Stale => WAIT,
                };
                // A hiccup keeps the last numbers; a sign-in that is really gone clears them.
                let (data, at) = match &self.last {
                    Some((d, at)) => (d.clone(), *at),
                    None => (Value::Null, now),
                };
                Limits {
                    status: why.as_str().into(),
                    data,
                    at,
                }
            }
        };
        (limits, self.wait)
    }
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut poll = Poll::new();
        loop {
            let (limits, wait) = poll.step(fetch(), now_ms());
            if let Some(latest) = app.try_state::<Latest>() {
                *latest.0.lock().unwrap() = limits.clone();
            }
            let _ = app.emit("limits", &limits);
            std::thread::sleep(Duration::from_secs(wait));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 429 keeps the last numbers and backs off up to the cap; success resets the pace;
    /// only a lost sign-in clears the numbers.
    #[test]
    fn busy_keeps_numbers_and_backs_off() {
        let mut p = Poll::new();
        let data = json!({ "five_hour": { "utilization": 42 } });

        let (l, w) = p.step(Err(Why::Busy), 1);
        assert_eq!(
            (l.status.as_str(), l.data.is_null(), w),
            ("busy", true, 120)
        );

        let (l, w) = p.step(Ok(data.clone()), 2);
        assert_eq!((l.status.as_str(), l.at, w), ("ok", 2, WAIT));

        let waits: Vec<u64> = (0..5)
            .map(|i| {
                let (l, w) = p.step(Err(Why::Busy), 3 + i);
                assert_eq!((l.status.as_str(), &l.data, l.at), ("busy", &data, 2));
                w
            })
            .collect();
        assert_eq!(waits, vec![120, 240, 480, 600, 600]);

        let (l, w) = p.step(Err(Why::Net), 10);
        assert_eq!((l.status.as_str(), &l.data, w), ("net", &data, 30));

        let (_, w) = p.step(Ok(data.clone()), 11);
        assert_eq!(w, WAIT);

        let (l, _) = p.step(Err(Why::Login), 12);
        assert_eq!((l.status.as_str(), l.data.is_null()), ("login", true));
        let (l, _) = p.step(Err(Why::Busy), 13);
        assert!(l.data.is_null());
    }

    #[test]
    fn status_codes() {
        assert!(status_why(429) == Why::Busy);
        assert!(status_why(529) == Why::Busy);
        assert!(status_why(401) == Why::Login);
        assert!(status_why(403) == Why::Login);
    }
}
