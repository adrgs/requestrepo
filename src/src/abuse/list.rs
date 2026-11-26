use crate::utils::auth::constant_time_eq;
use crate::utils::config::CONFIG;
use crate::utils::{get_current_timestamp, verify_subdomain};
use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::{limiter::ReportLimiter, storage::Storage};

/// Accepted reports per client per window.
pub const REPORT_RATE_LIMIT: u32 = 3;
pub const REPORT_RATE_WINDOW: StdDuration = StdDuration::from_secs(60 * 60);
/// How often expired rate-limit buckets are dropped.
const CLEANUP_INTERVAL: StdDuration = StdDuration::from_secs(60);

/// Upper bound on suspended subdomains (about 2.5 MB of file). The report endpoint is
/// anonymous and the whole file is rewritten on every change, so growth must have a limit.
/// When it is reached new reports are refused: lifting old suspensions to make room would
/// let a flood of reports bring a real abuser back.
const MAX_ENTRIES: usize = 50_000;

const FILE_VERSION: u32 = 1;

/// A suspended subdomain. Being on the list is being suspended; to lift a suspension,
/// remove its line from `abuse.json` and restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub subdomain: String,
    pub suspended_at: i64,
}

/// On-disk shape.
#[derive(Serialize, Deserialize)]
struct FileFormat {
    version: u32,
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(Default)]
struct Snapshot {
    entries: HashMap<String, Arc<Entry>>,
}

/// How the blocklist behaves. Comes from the environment in production.
#[derive(Debug, Clone)]
pub struct Settings {
    pub reporting_enabled: bool,
    /// ADMIN_TOKEN of a private instance. When set, reporting needs it, like every other
    /// way of using the instance does.
    pub admin_token: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            reporting_enabled: true,
            admin_token: None,
        }
    }
}

impl Settings {
    pub fn from_config() -> Self {
        Self {
            reporting_enabled: CONFIG.abuse_reporting_enabled,
            admin_token: CONFIG.admin_token.clone(),
        }
    }

    /// Whether a report carrying `token` may be filed.
    pub fn may_report(&self, token: Option<&str>) -> bool {
        match &self.admin_token {
            None => true,
            Some(required) => token.is_some_and(|token| constant_time_eq(token, required)),
        }
    }
}

/// The list of suspended subdomains.
///
/// Readers never wait on writers: they look at an immutable snapshot that is swapped in
/// whole, which keeps `is_blocked` cheap enough to run on every DNS query and HTTP request.
pub struct AbuseList {
    state: ArcSwap<Snapshot>,
    /// Held across clone-change-save-swap, so concurrent writers cannot overwrite each
    /// other's change with a copy taken before it.
    write_lock: Mutex<()>,
    storage: Storage,
    limiter: ReportLimiter,
    settings: Settings,
    persist_failures: AtomicU64,
}

impl AbuseList {
    /// Loads the blocklist from `data_dir`.
    ///
    /// Fails when a saved blocklist exists but cannot be read: starting without it would
    /// silently restore every suspended subdomain.
    pub async fn load(data_dir: impl AsRef<Path>, settings: Settings) -> Result<Self> {
        let storage = Storage::new(data_dir);
        let entries = match storage.load().await? {
            Some(text) => parse_file(&text, storage.path())?,
            None => Vec::new(),
        };
        if let Err(e) = storage.ensure_dir().await {
            error!("The blocklist cannot be saved and will be lost on restart: {e}");
        }

        info!(
            "Loaded {} suspended subdomain(s) from {}",
            entries.len(),
            storage.path().display()
        );

        let entries = entries
            .into_iter()
            .map(|entry| (entry.subdomain.clone(), Arc::new(entry)))
            .collect();
        Ok(Self {
            state: ArcSwap::new(Arc::new(Snapshot { entries })),
            write_lock: Mutex::new(()),
            storage,
            limiter: ReportLimiter::new(REPORT_RATE_LIMIT, REPORT_RATE_WINDOW),
            settings,
            persist_failures: AtomicU64::new(0),
        })
    }

    /// Whether `subdomain` is suspended. Lock-free.
    pub fn is_blocked(&self, subdomain: &str) -> bool {
        self.state.load().entries.contains_key(subdomain)
    }

    pub fn blocked_count(&self) -> usize {
        self.state.load().entries.len()
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Number of changes since startup that could not be written to disk.
    pub fn persist_failures(&self) -> u64 {
        self.persist_failures.load(Ordering::Relaxed)
    }

    /// Applies the report rate limit for `ip`.
    pub fn check_report_rate(&self, ip: IpAddr) -> Result<(), StdDuration> {
        self.limiter.check(ip)
    }

    /// Starts dropping expired rate-limit buckets. Without it, every client that ever
    /// reported would be remembered for the life of the process.
    pub fn start_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(CLEANUP_INTERVAL).await;
                self.limiter.sweep();
            }
        });
    }

    /// Suspends `subdomain`: `Some(true)` if it now is, `Some(false)` if it already was,
    /// `None` if the list is full.
    pub async fn suspend(&self, subdomain: &str) -> Option<bool> {
        if self.is_blocked(subdomain) {
            return Some(false);
        }
        let now = get_current_timestamp();
        self.update(|snapshot| {
            if snapshot.entries.contains_key(subdomain) {
                return Some(false);
            }
            if snapshot.entries.len() >= MAX_ENTRIES {
                error!("Blocklist full ({MAX_ENTRIES} entries): refusing to suspend {subdomain}");
                return None;
            }
            let entry = Entry {
                subdomain: subdomain.to_string(),
                suspended_at: now,
            };
            snapshot
                .entries
                .insert(subdomain.to_string(), Arc::new(entry));
            Some(true)
        })
        .await
    }

    /// Applies `change` to a copy of the current snapshot, saves it, and publishes it.
    ///
    /// A failed save does not undo the change. It takes effect now and is reported, so a
    /// disk problem can never stop an abusive subdomain from being suspended.
    async fn update<T>(&self, change: impl FnOnce(&mut Snapshot) -> T) -> T {
        let _writer = self.write_lock.lock().await;

        let mut next = Snapshot {
            entries: self.state.load().entries.clone(),
        };
        let value = change(&mut next);

        let contents = encode(&next);
        if let Err(e) = self.storage.save(&contents).await {
            self.persist_failures.fetch_add(1, Ordering::Relaxed);
            error!("The blocklist cannot be saved and will be lost on restart: {e}");
        }
        self.state.store(Arc::new(next));
        value
    }
}

/// Serialises a snapshot, one entry per line so that lifting a suspension by hand is a
/// matter of deleting a line. Sorted, so an unchanged list rewrites the same bytes.
fn encode(snapshot: &Snapshot) -> Vec<u8> {
    let mut entries: Vec<&Entry> = snapshot.entries.values().map(Arc::as_ref).collect();
    entries.sort_unstable_by(|a, b| a.subdomain.cmp(&b.subdomain));
    let lines: Vec<String> = entries
        .iter()
        .map(|entry| serde_json::to_string(entry).expect("an entry always serialises"))
        .collect();
    format!(
        "{{\"version\": {FILE_VERSION}, \"entries\": [\n{}\n]}}\n",
        lines.join(",\n")
    )
    .into_bytes()
}

/// Drops a comma that directly precedes `]` or `}`, so that deleting the last line of the
/// entry list by hand does not leave invalid JSON behind. Commas inside strings are kept:
/// subdomain labels never contain brackets, and nothing else in the file is free text.
fn without_trailing_commas(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_comma = None;
    for c in text.chars() {
        match c {
            ',' => {
                if let Some(comma) = pending_comma.replace(String::from(",")) {
                    out.push_str(&comma);
                }
            }
            c if c.is_whitespace() => match pending_comma.as_mut() {
                Some(pending) => pending.push(c),
                None => out.push(c),
            },
            ']' | '}' => {
                if let Some(pending) = pending_comma.take() {
                    out.push_str(pending.trim_start_matches(','));
                }
                out.push(c);
            }
            c => {
                if let Some(pending) = pending_comma.take() {
                    out.push_str(&pending);
                }
                out.push(c);
            }
        }
    }
    if let Some(pending) = pending_comma {
        out.push_str(&pending);
    }
    out
}

/// Parses and validates a saved blocklist. A malformed file is a version mismatch or an
/// editing mistake, and must stop startup rather than drop suspensions.
fn parse_file(text: &str, path: &Path) -> Result<Vec<Entry>> {
    let text = without_trailing_commas(text);
    let file: FileFormat = serde_json::from_str(&text)
        .with_context(|| format!("Blocklist {} is not valid JSON", path.display()))?;
    if file.version != FILE_VERSION {
        return Err(anyhow::anyhow!(
            "Blocklist {} has version {}, expected {FILE_VERSION}",
            path.display(),
            file.version
        ));
    }

    // An entry that does not fit the current SUBDOMAIN_LENGTH / SUBDOMAIN_ALPHABET can no
    // longer be reached either, so it is skipped rather than allowed to stop startup (and,
    // on an instance that is its own nameserver, take the domain down with it).
    let (entries, unreachable): (Vec<Entry>, Vec<Entry>) =
        file.entries.into_iter().partition(|entry| {
            verify_subdomain(
                &entry.subdomain,
                CONFIG.subdomain_length,
                &CONFIG.subdomain_alphabet_set,
            )
        });
    for entry in &unreachable {
        warn!(
            "Blocklist {}: ignoring '{}', which is not a subdomain under the current settings",
            path.display(),
            entry.subdomain
        );
    }

    let mut seen = HashSet::new();
    for entry in &entries {
        if !seen.insert(entry.subdomain.as_str()) {
            return Err(anyhow::anyhow!(
                "Blocklist {} lists '{}' more than once",
                path.display(),
                entry.subdomain
            ));
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn load(dir: &TempDir) -> AbuseList {
        AbuseList::load(dir.path(), Settings::default())
            .await
            .unwrap()
    }

    fn label(n: usize) -> String {
        format!("{n:08}")
    }

    #[tokio::test]
    async fn starts_empty_when_nothing_was_saved() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir).await;

        assert!(!list.is_blocked("abcdefgh"));
        assert_eq!(list.blocked_count(), 0);
    }

    #[tokio::test]
    async fn suspends_once_and_persists() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir).await;

        assert_eq!(list.suspend("abcdefgh").await, Some(true));
        assert_eq!(list.persist_failures(), 0);
        assert!(list.is_blocked("abcdefgh"));
        assert_eq!(list.suspend("abcdefgh").await, Some(false));

        // A restart must not bring the subdomain back
        assert!(load(&dir).await.is_blocked("abcdefgh"));
    }

    #[tokio::test]
    async fn the_file_has_one_entry_per_line_so_it_can_be_edited_by_hand() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir).await;
        list.suspend("aaaaaaaa").await;
        list.suspend("bbbbbbbb").await;

        let path = dir.path().join("abuse.json");
        let text = std::fs::read_to_string(&path).unwrap();
        // Deleting the first line's entry leaves valid JSON once its comma goes too
        let edited = text.replace(
            &format!("{},\n", text.lines().nth(1).unwrap().trim_end_matches(',')),
            "",
        );
        std::fs::write(&path, edited).unwrap();

        let reloaded = load(&dir).await;
        assert!(!reloaded.is_blocked("aaaaaaaa"));
        assert!(reloaded.is_blocked("bbbbbbbb"));
    }

    #[tokio::test]
    async fn deleting_the_last_line_by_hand_is_fine_too() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir).await;
        list.suspend("aaaaaaaa").await;
        list.suspend("bbbbbbbb").await;

        let path = dir.path().join("abuse.json");
        let text = std::fs::read_to_string(&path).unwrap();
        let edited: Vec<&str> = text
            .lines()
            .filter(|line| !line.contains("bbbbbbbb"))
            .collect();
        std::fs::write(&path, edited.join("\n")).unwrap();

        let reloaded = load(&dir).await;
        assert!(reloaded.is_blocked("aaaaaaaa"));
        assert!(!reloaded.is_blocked("bbbbbbbb"));
    }

    #[test]
    fn trailing_commas_are_dropped_and_nothing_else_changes() {
        assert_eq!(without_trailing_commas("[1, 2,\n]"), "[1, 2\n]");
        assert_eq!(without_trailing_commas("{\"a\": 1, }"), "{\"a\": 1 }");
        assert_eq!(without_trailing_commas("[1, 2]"), "[1, 2]");
        assert_eq!(without_trailing_commas("[1,\n 2]"), "[1,\n 2]");
    }

    #[tokio::test]
    async fn a_full_list_refuses_new_suspensions_and_keeps_the_old_ones() {
        let dir = TempDir::new().unwrap();
        let entries: Vec<String> = (0..MAX_ENTRIES)
            .map(|n| format!(r#"{{"subdomain": "{}", "suspended_at": {n}}}"#, label(n)))
            .collect();
        std::fs::write(
            dir.path().join("abuse.json"),
            format!(r#"{{"version": 1, "entries": [{}]}}"#, entries.join(",")),
        )
        .unwrap();
        let list = load(&dir).await;

        assert_eq!(list.suspend("newcomer").await, None);
        assert!(!list.is_blocked("newcomer"));
        assert!(list.is_blocked(&label(0)), "nothing is lifted to make room");
        assert_eq!(list.blocked_count(), MAX_ENTRIES);
        // Reporting one that is already suspended is still fine
        assert_eq!(list.suspend(&label(0)).await, Some(false));
    }

    #[tokio::test]
    async fn concurrent_suspensions_are_all_kept() {
        let dir = TempDir::new().unwrap();
        let list = Arc::new(load(&dir).await);

        let mut tasks = Vec::new();
        for n in 0..64 {
            let list = list.clone();
            tasks.push(tokio::spawn(async move { list.suspend(&label(n)).await }));
        }
        for task in tasks {
            task.await.unwrap();
        }

        assert_eq!(list.blocked_count(), 64);
        assert_eq!(load(&dir).await.blocked_count(), 64);
    }

    #[tokio::test]
    async fn failed_save_still_takes_effect_and_is_counted() {
        let dir = TempDir::new().unwrap();
        // A directory where the temp file would be created makes every save fail
        std::fs::create_dir(dir.path().join("abuse.json.tmp")).unwrap();
        let list = load(&dir).await;

        assert_eq!(list.suspend("abcdefgh").await, Some(true));
        assert!(list.is_blocked("abcdefgh"));
        assert_eq!(list.persist_failures(), 1);
    }

    #[tokio::test]
    async fn corrupt_files_refuse_to_load_and_unreachable_labels_are_skipped() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("abuse.json");
        let entry = r#"{"subdomain": "abcdefgh", "suspended_at": 0}"#;

        let cases: Vec<(String, &str)> = vec![
            ("not json".to_string(), "not valid JSON"),
            (
                "{\"version\": 2, \"entries\": []}".to_string(),
                "expected 1",
            ),
            (
                format!("{{\"version\": 1, \"entries\": [{entry}, {entry}]}}"),
                "more than once",
            ),
        ];
        for (contents, expected) in cases {
            std::fs::write(&path, &contents).unwrap();
            let err = AbuseList::load(dir.path(), Settings::default())
                .await
                .err()
                .unwrap();
            assert!(err.to_string().contains(expected), "{contents}: {err}");
        }

        // An entry that no longer fits the subdomain settings is skipped, not fatal
        std::fs::write(
            &path,
            format!(
                "{{\"version\": 1, \"entries\": [{entry}, {}]}}",
                entry.replace("abcdefgh", "not-a-label")
            ),
        )
        .unwrap();
        let list = load(&dir).await;
        assert_eq!(list.blocked_count(), 1);
        assert!(list.is_blocked("abcdefgh"));

        // An entry list is optional
        std::fs::write(&path, r#"{"version": 1}"#).unwrap();
        assert_eq!(load(&dir).await.blocked_count(), 0);
    }
}
