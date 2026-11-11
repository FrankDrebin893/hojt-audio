//! The update check: is there a newer release of the plugins than the one running?
//!
//! No UI here (the editors show a line from a [`Notice`]). The check asks the GitHub releases API
//! for the newest release that is not a pre-release and has a tag `vX.Y.Z`, and compares it with
//! the running version. It runs at most once a day, on a thread of its own that is started outside
//! the audio thread and never touches it, with a short timeout. Offline, rate limited or any
//! other failure is silent and tried again the next day. It sends nothing but the request: a
//! `HojtDSP/<version>` User-Agent and the `Accept` header, no id, no cookie, no body.
//!
//! What is kept lives in the plugin's `#[persist]` state, never a parameter, so it does not show
//! in a DAW and has no parameter id:
//!
//! ```ignore
//! #[persist = "update"]
//! update: Arc<Mutex<suite_common::update::UpdateSettings>>,
//! ```
//!
//! The key is the plugin's to list on its issue and frozen once shipped. An editor reads the
//! result through an [`UpdateCheck`] handle: [`UpdateCheck::notice`] for the line, and
//! [`UpdateCheck::on_result`] to ask the host for a repaint in the frame a result arrives (an
//! editor paints only when asked, `ui/STYLE.md`).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The releases of the repository, newest first, as the GitHub API serves them
pub const RELEASES_URL: &str =
    "https://api.github.com/repos/FrankDrebin893/hojt-audio/releases?per_page=30";

/// Where a player downloads the newest release
pub const DOWNLOAD_URL: &str = "https://hojte.net/hojt-audio/dsp/";

/// How long the check waits for the whole answer
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// The least time between two checks
pub const INTERVAL_SECS: u64 = 24 * 60 * 60;

/// The running version, from the workspace's `Cargo.toml`
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// A release version `major.minor.patch`; it orders as a version does
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// Reads `X.Y.Z` or `vX.Y.Z`. Anything else is `None`: a pre-release (`v1.2.3-beta`), build
    /// metadata, a missing or extra part, a sign, a space, or another product's tag
    /// (`piano-v1.0.0`)
    pub fn parse(tag: &str) -> Option<Version> {
        let s = tag.strip_prefix('v').unwrap_or(tag);
        let mut parts = s.split('.');
        let mut next = || {
            let p = parts.next()?;
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            p.parse::<u32>().ok()
        };
        let v = Version(next()?, next()?, next()?);
        parts.next().is_none().then_some(v)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// What is kept between sessions, in the plugin's `#[persist]` state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    /// The off switch. On by default
    pub enabled: bool,
    /// When the last check was tried, in seconds since 1970 (0: never), whether it worked or not
    pub last_check: u64,
    /// The newest version the last working check found, `X.Y.Z`; empty when none has
    pub latest: String,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        UpdateSettings { enabled: true, last_check: 0, latest: String::new() }
    }
}

impl UpdateSettings {
    /// Whether a check should be made at `now` (seconds since 1970): it is on, and the last try
    /// was a day ago or more. A last try in the future (the clock was set back) counts as due
    pub fn due(&self, now: u64) -> bool {
        self.enabled && (self.last_check > now || now - self.last_check >= INTERVAL_SECS)
    }
}

/// What an editor shows a line from
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub current: Version,
    pub latest: Version,
    pub url: &'static str,
}

/// Why a check did not give a release. Never shown: a failed check is silent
#[derive(Debug)]
pub enum Error {
    /// No connection, a timeout, or a broken answer
    Network(String),
    /// The server answered with something other than 200 (403 rate limited, 404, ...)
    Status(i32),
    /// The answer is not the releases list
    BadJson,
    /// The list has no release of ours that is not a pre-release
    NoRelease,
}

/// The newest release in the API's answer: the highest `vX.Y.Z` tag of those that are neither
/// drafts nor pre-releases. Releases of the piano viz (`piano-v...`) are not ours
pub fn newest_release(json: &[u8]) -> Result<Version, Error> {
    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
        #[serde(default)]
        draft: bool,
        #[serde(default)]
        prerelease: bool,
    }
    let releases: Vec<Release> = serde_json::from_slice(json).map_err(|_| Error::BadJson)?;
    releases
        .iter()
        .filter(|r| !r.draft && !r.prerelease && r.tag_name.starts_with('v'))
        .filter_map(|r| Version::parse(&r.tag_name))
        .max()
        .ok_or(Error::NoRelease)
}

/// Makes the one request, to `url`, and reads the newest release from the answer. This is the
/// whole of what is sent: a GET with `User-Agent: HojtDSP/<version>` and `Accept`
pub fn fetch_latest(url: &str, timeout: Duration) -> Result<Version, Error> {
    let response = minreq::get(url)
        .with_header("User-Agent", format!("HojtDSP/{CURRENT}"))
        .with_header("Accept", "application/vnd.github+json")
        .with_timeout(timeout.as_secs().max(1))
        .send()
        .map_err(|e| Error::Network(e.to_string()))?;
    if response.status_code != 200 {
        return Err(Error::Status(response.status_code));
    }
    newest_release(response.as_bytes())
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn lock(settings: &Mutex<UpdateSettings>) -> MutexGuard<'_, UpdateSettings> {
    settings.lock().unwrap_or_else(|e| e.into_inner())
}

type Repaint = Box<dyn Fn() + Send + Sync>;

struct Shared {
    settings: Arc<Mutex<UpdateSettings>>,
    /// Set when a result arrived and the editor has not drawn it yet
    fresh: AtomicBool,
    repaint: Mutex<Option<Repaint>>,
}

/// The handle an editor holds. Cheap to clone; reading it never blocks on the network
#[derive(Clone)]
pub struct UpdateCheck {
    shared: Arc<Shared>,
}

impl UpdateCheck {
    /// Starts the check for a plugin whose persisted state is `settings`: if it is on and a
    /// day has passed, a background thread makes the one request. Call it where the editor is
    /// made, never from `process`
    pub fn start(settings: Arc<Mutex<UpdateSettings>>) -> UpdateCheck {
        Self::start_with(settings, RELEASES_URL, TIMEOUT, now_secs())
    }

    /// [`start`](Self::start) with the address, the timeout and the time given: for tests
    pub fn start_with(
        settings: Arc<Mutex<UpdateSettings>>,
        url: &str,
        timeout: Duration,
        now: u64,
    ) -> UpdateCheck {
        let shared =
            Arc::new(Shared { settings, fresh: AtomicBool::new(false), repaint: Mutex::new(None) });
        {
            // The attempt is noted before it is made, so a crash or a slow answer does not
            // make every window open try again
            let mut s = lock(&shared.settings);
            if !s.due(now) {
                drop(s);
                return UpdateCheck { shared };
            }
            s.last_check = now;
        }
        let worker = shared.clone();
        let url = url.to_string();
        let spawned =
            std::thread::Builder::new().name("hojt-update-check".into()).spawn(move || {
                if let Ok(latest) = fetch_latest(&url, timeout) {
                    lock(&worker.settings).latest = latest.to_string();
                    worker.fresh.store(true, Ordering::Release);
                    if let Some(f) =
                        worker.repaint.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
                    {
                        f();
                    }
                }
            });
        drop(spawned);
        UpdateCheck { shared }
    }

    /// Asks `f` to be called when a result arrives, from the check's thread: the editor passes
    /// something that requests a repaint. Replaces an earlier one. If a result already arrived
    /// it is called at once
    pub fn on_result(&self, f: impl Fn() + Send + Sync + 'static) {
        if self.shared.fresh.load(Ordering::Acquire) {
            f();
        }
        *self.shared.repaint.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
    }

    /// Whether a result arrived since the last call. The editor that draws on request asks this
    /// in a frame it was woken for
    pub fn take_fresh(&self) -> bool {
        self.shared.fresh.swap(false, Ordering::AcqRel)
    }

    /// The line to show: `Some` only when the check is on and the newest known version is newer
    /// than the running one. From the persisted result, so it shows at once on the day after
    pub fn notice(&self) -> Option<Notice> {
        let s = lock(&self.shared.settings);
        if !s.enabled {
            return None;
        }
        notice_for(&s.latest, CURRENT)
    }
}

/// The notice for a stored latest version against the running one
pub fn notice_for(latest: &str, current: &str) -> Option<Notice> {
    let latest = Version::parse(latest)?;
    let current = Version::parse(current)?;
    (latest > current).then_some(Notice { current, latest, url: DOWNLOAD_URL })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Instant;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn versions_parse_and_order() {
        assert_eq!(v("v0.3.0"), Version(0, 3, 0));
        assert_eq!(v("1.2.3"), Version(1, 2, 3));
        assert!(v("v0.10.0") > v("v0.9.9"));
        assert!(v("v1.0.0") > v("v0.99.99"));
        assert_eq!(v("v0.3.0"), v("0.3.0"));
    }

    #[test]
    fn malformed_tags_are_none() {
        for t in [
            "",
            "v",
            "v1",
            "v1.2",
            "v1.2.3.4",
            "v1.2.3-beta",
            "v1.2.3+7",
            "piano-v1.0.0",
            "vx.y.z",
            "v1..3",
            "v-1.2.3",
            "v1.2.+3",
            " v1.2.3",
            "V1.2.3",
            "latest",
        ] {
            assert_eq!(Version::parse(t), None, "{t:?}");
        }
    }

    #[test]
    fn notice_equal_newer_older() {
        assert_eq!(notice_for("0.3.0", "0.3.0"), None);
        assert_eq!(notice_for("0.2.9", "0.3.0"), None);
        let n = notice_for("0.4.0", "0.3.0").unwrap();
        assert_eq!((n.current, n.latest), (Version(0, 3, 0), Version(0, 4, 0)));
        assert_eq!(n.url, DOWNLOAD_URL);
        assert_eq!(notice_for("", "0.3.0"), None);
        assert_eq!(notice_for("garbage", "0.3.0"), None);
    }

    #[test]
    fn the_running_version_is_a_release_version() {
        assert!(Version::parse(CURRENT).is_some());
    }

    #[test]
    fn newest_release_ignores_prereleases_drafts_and_the_piano() {
        let json = br#"[
          {"tag_name":"v9.0.0","prerelease":true,"draft":false},
          {"tag_name":"v8.0.0","prerelease":false,"draft":true},
          {"tag_name":"piano-v7.0.0","prerelease":false,"draft":false},
          {"tag_name":"v1.2.3-rc1","prerelease":false,"draft":false},
          {"tag_name":"v0.9.0","prerelease":false,"draft":false},
          {"tag_name":"v0.10.0","prerelease":false,"draft":false,"name":"x"},
          {"tag_name":"v0.3.0","prerelease":false,"draft":false}
        ]"#;
        assert_eq!(newest_release(json).unwrap(), Version(0, 10, 0));
    }

    #[test]
    fn newest_release_failures() {
        assert!(matches!(newest_release(b"not json"), Err(Error::BadJson)));
        assert!(matches!(newest_release(b"{}"), Err(Error::BadJson)));
        assert!(matches!(newest_release(b"[]"), Err(Error::NoRelease)));
        assert!(matches!(
            newest_release(br#"[{"tag_name":"v1.0.0","prerelease":true}]"#),
            Err(Error::NoRelease)
        ));
    }

    #[test]
    fn gating_is_daily() {
        let day = INTERVAL_SECS;
        let mut s = UpdateSettings::default();
        assert!(s.enabled);
        assert!(s.due(1_000_000), "never checked");
        s.last_check = 1_000_000;
        assert!(!s.due(1_000_000));
        assert!(!s.due(1_000_000 + day - 1));
        assert!(s.due(1_000_000 + day));
        assert!(s.due(500_000), "a clock set back");
        s.enabled = false;
        assert!(!s.due(1_000_000 + 10 * day));
    }

    #[test]
    fn settings_persist_with_defaults_for_missing_fields() {
        let s = UpdateSettings { enabled: false, last_check: 5, latest: "0.4.0".into() };
        let back: UpdateSettings =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        let old: UpdateSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(old, UpdateSettings::default());
    }

    /// A one-shot server on a free port: answers the first connection with `reply` after `delay`,
    /// and hands back the request it read
    fn serve(reply: &'static str, delay: Duration) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/releases", listener.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let (mut c, _) = listener.accept().unwrap();
            let mut req = Vec::new();
            let mut buf = [0u8; 1024];
            while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                match c.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => req.extend_from_slice(&buf[..n]),
                }
            }
            std::thread::sleep(delay);
            let _ = c.write_all(reply.as_bytes());
            String::from_utf8_lossy(&req).into_owned()
        });
        (url, h)
    }

    fn ok(body: &str) -> &'static str {
        Box::leak(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .into_boxed_str(),
        )
    }

    #[test]
    fn the_request_is_a_plain_get_with_our_user_agent() {
        let (url, server) = serve(ok(r#"[{"tag_name":"v0.4.0"}]"#), Duration::ZERO);
        let got = fetch_latest(&url, TIMEOUT).unwrap();
        assert_eq!(got, Version(0, 4, 0));
        let req = server.join().unwrap();
        let mut lines: Vec<String> = req.lines().map(|l| l.to_ascii_lowercase()).collect();
        // Print it so the issue can show exactly what is sent
        eprintln!("--- request ---\n{req}");
        assert!(lines[0].starts_with("get /releases http/1.1"));
        assert!(req.contains(&format!("User-Agent: HojtDSP/{CURRENT}")));
        lines.retain(|l| !l.is_empty());
        for l in &lines[1..] {
            let name = l.split(':').next().unwrap();
            assert!(["host", "user-agent", "accept"].contains(&name), "unexpected header {l}");
        }
        assert!(!lines.iter().any(|l| l.starts_with("cookie") || l.starts_with("content-length")));
    }

    #[test]
    fn failure_paths_are_errors_not_panics() {
        let (url, s) = serve(
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            Duration::ZERO,
        );
        assert!(matches!(fetch_latest(&url, TIMEOUT), Err(Error::Status(403))));
        s.join().unwrap();

        let (url, s) = serve(
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            Duration::ZERO,
        );
        assert!(matches!(fetch_latest(&url, TIMEOUT), Err(Error::Status(404))));
        s.join().unwrap();

        let (url, s) = serve(ok("<html>"), Duration::ZERO);
        assert!(matches!(fetch_latest(&url, TIMEOUT), Err(Error::BadJson)));
        s.join().unwrap();

        // Timeout: the server waits longer than the client does
        let (url, s) = serve(ok("[]"), Duration::from_millis(2500));
        let t = Instant::now();
        assert!(matches!(fetch_latest(&url, Duration::from_secs(1)), Err(Error::Network(_))));
        assert!(t.elapsed() < Duration::from_millis(2400));
        s.join().unwrap();

        // Nothing listening
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}/");
        assert!(matches!(fetch_latest(&url, TIMEOUT), Err(Error::Network(_))));
    }

    fn settings(enabled: bool, last_check: u64) -> Arc<Mutex<UpdateSettings>> {
        Arc::new(Mutex::new(UpdateSettings { enabled, last_check, latest: String::new() }))
    }

    fn wait_for(h: &UpdateCheck) {
        let t = Instant::now();
        while !h.shared.fresh.load(Ordering::Acquire) && t.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_check_stores_the_result_and_asks_for_a_repaint() {
        let (url, server) = serve(ok(r#"[{"tag_name":"v99.0.0"}]"#), Duration::ZERO);
        let st = settings(true, 0);
        let h = UpdateCheck::start_with(st.clone(), &url, TIMEOUT, 1_000_000);
        let woken = Arc::new(AtomicBool::new(false));
        let w = woken.clone();
        h.on_result(move || w.store(true, Ordering::SeqCst));
        wait_for(&h);
        server.join().unwrap();
        assert!(h.take_fresh());
        assert!(!h.take_fresh());
        // The callback may have been registered after the result: on_result calls it then
        assert!(woken.load(Ordering::SeqCst) || h.notice().is_some());
        let s = lock(&st);
        assert_eq!((s.last_check, s.latest.as_str()), (1_000_000, "99.0.0"));
        drop(s);
        let n = h.notice().unwrap();
        assert_eq!(n.latest, Version(99, 0, 0));
    }

    #[test]
    fn a_failed_check_is_silent_and_waits_a_day() {
        let (url, server) = serve(
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            Duration::ZERO,
        );
        let st = settings(true, 0);
        lock(&st).latest = "0.0.1".into();
        let h = UpdateCheck::start_with(st.clone(), &url, TIMEOUT, 2_000_000);
        server.join().unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert!(!h.take_fresh());
        let s = lock(&st);
        assert_eq!(s.last_check, 2_000_000, "the try is noted");
        assert_eq!(s.latest, "0.0.1", "the old result stays");
        assert!(!s.due(2_000_000 + INTERVAL_SECS - 1));
    }

    /// Whether anything connected to `listener` in the next moments
    fn nobody_connects(listener: &TcpListener) -> bool {
        listener.set_nonblocking(true).unwrap();
        std::thread::sleep(Duration::from_millis(400));
        listener.accept().is_err()
    }

    #[test]
    fn with_the_check_off_no_connection_is_opened() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let st = settings(false, 0);
        let h = UpdateCheck::start_with(st.clone(), &url, TIMEOUT, 1_000_000);
        assert!(nobody_connects(&listener));
        assert!(!h.take_fresh());
        assert_eq!(lock(&st).last_check, 0);
        assert!(h.notice().is_none());
    }

    #[test]
    fn within_a_day_no_connection_is_opened() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let st = settings(true, 1_000_000);
        UpdateCheck::start_with(st, &url, TIMEOUT, 1_000_000 + 60);
        assert!(nobody_connects(&listener));
    }

    #[test]
    fn off_hides_a_stored_notice() {
        let st = Arc::new(Mutex::new(UpdateSettings {
            enabled: false,
            last_check: 0,
            latest: "99.0.0".into(),
        }));
        let h = UpdateCheck::start_with(st.clone(), "http://127.0.0.1:1/", TIMEOUT, 5);
        assert!(h.notice().is_none());
        lock(&st).enabled = true;
        assert!(h.notice().is_some());
    }
}
