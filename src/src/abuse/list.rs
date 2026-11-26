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

/// Accepted reports per client network per window.
pub const REPORT_RATE_LIMIT: u32 = 3;
pub const REPORT_RATE_WINDOW: StdDuration = StdDuration::from_secs(60 * 60);
/// How often expired rate-limit buckets are dropped.
const CLEANUP_INTERVAL: StdDuration = StdDuration::from_secs(60);

/// Upper bound on suspended subdomains. The whole file is rewritten on every change and the
/// report endpoint is anonymous, so growth must have a limit. When it is reached, the
/// oldest suspension made by a report is lifted to make room; an admin's never is.
const MAX_ENTRIES: usize = 5_000;

/// Reports kept per entry, most recent first. `report_count` holds the true total.
const REPORTS_PER_ENTRY: usize = 3;

const FILE_VERSION: u32 = 1;

/// Why a subdomain was suspended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Malware,
    Phishing,
    Csam,
    Spam,
    Other,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Malware,
        Category::Phishing,
        Category::Csam,
        Category::Spam,
        Category::Other,
    ];
}

/// Who suspended the subdomain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Report,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub at: i64,
    pub category: Category,
    /// Reporter's network, such as `203.0.113.0/24`. The exact address is not kept.
    pub network: String,
    /// Optional short reason, shown to the admin only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A suspended subdomain. Being on the list is being suspended; lifting removes the entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub subdomain: String,
    pub category: Category,
    pub source: Source,
    pub suspended_at: i64,
    pub updated_at: i64,
    pub report_count: u32,
    #[serde(default)]
    pub reports: Vec<Report>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_note: Option<String>,
}

/// On-disk shape.
#[derive(Serialize, Deserialize)]
struct FileFormat {
    version: u32,
    #[serde(default)]
    entries: Vec<Entry>,
}

/// Borrows from a snapshot, so saving does not have to clone every entry.
#[derive(Serialize)]
struct FileFormatRef<'a> {
    version: u32,
    entries: Vec<&'a Entry>,
}

#[derive(Default)]
struct Snapshot {
    entries: HashMap<String, Arc<Entry>>,
}

impl Snapshot {
    /// Makes room for one more entry by lifting the oldest suspension made by a report.
    /// Suspensions an admin made are never lifted this way.
    fn make_room(&mut self) -> bool {
        if self.entries.len() < MAX_ENTRIES {
            return true;
        }
        let oldest = self
            .entries
            .values()
            .filter(|entry| entry.source == Source::Report)
            .min_by_key(|entry| entry.updated_at)
            .map(|entry| entry.subdomain.clone());
        match oldest {
            Some(subdomain) => {
                warn!("Blocklist full: lifting the oldest reported suspension, {subdomain}");
                self.entries.remove(&subdomain);
                true
            }
            None => false,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MutateError {
    /// Every entry is an admin's suspension, so nothing can make room.
    Full,
}

/// Result of a change. `persisted` is false when the change took effect in memory but could
/// not be written to disk, in which case it is lost on restart.
#[derive(Debug)]
pub struct Applied<T> {
    pub value: T,
    pub persisted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminError {
    /// No admin token is configured, so there is nobody allowed to manage the blocklist.
    NotConfigured,
    Invalid,
}

/// How the blocklist behaves. Comes from the environment in production.
#[derive(Debug, Clone)]
pub struct Settings {
    pub reporting_enabled: bool,
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
            admin_token: CONFIG.abuse_admin_token.clone(),
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

    /// The entry for `subdomain`, if it is suspended.
    pub fn suspension(&self, subdomain: &str) -> Option<Arc<Entry>> {
        self.state.load().entries.get(subdomain).cloned()
    }

    /// Every entry, in no particular order.
    pub fn entries(&self) -> Vec<Arc<Entry>> {
        self.state.load().entries.values().cloned().collect()
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

    /// Checks the token supplied with an admin request.
    pub fn verify_admin(&self, supplied: Option<&str>) -> Result<(), AdminError> {
        let configured = self
            .settings
            .admin_token
            .as_deref()
            .ok_or(AdminError::NotConfigured)?;
        match supplied {
            Some(supplied) if constant_time_eq(supplied, configured) => Ok(()),
            _ => Err(AdminError::Invalid),
        }
    }

    /// Applies the report rate limit for `ip`.
    pub fn check_report_rate(&self, ip: IpAddr) -> Result<(), StdDuration> {
        self.limiter.check(ip)
    }

    /// Starts dropping expired rate-limit buckets. Without it, every network that ever
    /// reported would be remembered for the life of the process.
    pub fn start_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(CLEANUP_INTERVAL).await;
                self.limiter.sweep();
            }
        });
    }

    /// Suspends `subdomain` because of a report, or adds the report to its suspension. The
    /// value is true if this report is what suspended it.
    pub async fn report(
        &self,
        subdomain: &str,
        report: Report,
    ) -> Result<Applied<bool>, MutateError> {
        self.update(|snapshot| {
            let (mut entry, newly) = match snapshot.entries.get(subdomain) {
                Some(existing) => ((**existing).clone(), false),
                None => {
                    if !snapshot.make_room() {
                        return None;
                    }
                    let entry = Entry {
                        subdomain: subdomain.to_string(),
                        category: report.category,
                        source: Source::Report,
                        suspended_at: report.at,
                        updated_at: report.at,
                        report_count: 0,
                        reports: Vec::new(),
                        admin_note: None,
                    };
                    (entry, true)
                }
            };
            entry.report_count = entry.report_count.saturating_add(1);
            entry.updated_at = report.at;
            entry.reports.insert(0, report);
            entry.reports.truncate(REPORTS_PER_ENTRY);
            snapshot
                .entries
                .insert(subdomain.to_string(), Arc::new(entry));
            Some(newly)
        })
        .await
        .ok_or(MutateError::Full)
    }

    /// Suspends `subdomain` as an admin. The value is false if it already was suspended; a
    /// category or note given then still replaces the old one, and the suspension becomes
    /// the admin's, so the full list never lifts it.
    pub async fn suspend(
        &self,
        subdomain: &str,
        category: Option<Category>,
        note: Option<String>,
    ) -> Result<Applied<bool>, MutateError> {
        let now = get_current_timestamp();
        self.update(|snapshot| {
            let (mut entry, newly) = match snapshot.entries.get(subdomain) {
                Some(existing) => ((**existing).clone(), false),
                None => {
                    if !snapshot.make_room() {
                        return None;
                    }
                    let entry = Entry {
                        subdomain: subdomain.to_string(),
                        category: category.unwrap_or(Category::Other),
                        source: Source::Admin,
                        suspended_at: now,
                        updated_at: now,
                        report_count: 0,
                        reports: Vec::new(),
                        admin_note: None,
                    };
                    (entry, true)
                }
            };
            entry.source = Source::Admin;
            entry.updated_at = now;
            if let Some(category) = category {
                entry.category = category;
            }
            if note.is_some() {
                entry.admin_note = note;
            }
            snapshot
                .entries
                .insert(subdomain.to_string(), Arc::new(entry));
            Some(newly)
        })
        .await
        .ok_or(MutateError::Full)
    }

    /// Lifts the suspension of `subdomain`. The value is false if it was not suspended.
    pub async fn lift(&self, subdomain: &str) -> Applied<bool> {
        self.update(|snapshot| Some(snapshot.entries.remove(subdomain).is_some()))
            .await
            .expect("lifting a suspension cannot be refused")
    }

    /// Applies `change` to a copy of the current snapshot, saves it, and publishes it.
    /// A `None` result abandons the change without saving.
    ///
    /// A failed save does not undo the change. It takes effect now and is reported, so a
    /// disk problem can never stop an abusive subdomain from being suspended.
    async fn update<T>(
        &self,
        change: impl FnOnce(&mut Snapshot) -> Option<T>,
    ) -> Option<Applied<T>> {
        let _writer = self.write_lock.lock().await;

        let mut next = Snapshot {
            entries: self.state.load().entries.clone(),
        };
        let value = change(&mut next)?;

        // Encoding a large list is CPU work: keep it off the async workers.
        let next = Arc::new(next);
        let contents = {
            let next = next.clone();
            tokio::task::spawn_blocking(move || encode(&next))
                .await
                .expect("encoding the blocklist cannot panic")
        };
        let persisted = match self.storage.save(&contents).await {
            Ok(()) => true,
            Err(e) => {
                self.persist_failures.fetch_add(1, Ordering::Relaxed);
                error!("The blocklist cannot be saved and will be lost on restart: {e}");
                false
            }
        };
        self.state.store(next);

        Some(Applied { value, persisted })
    }
}

/// Serialises a snapshot. Entries are sorted so that saving an unchanged blocklist rewrites
/// the same bytes.
fn encode(snapshot: &Snapshot) -> Vec<u8> {
    let mut entries: Vec<&Entry> = snapshot.entries.values().map(Arc::as_ref).collect();
    entries.sort_unstable_by(|a, b| a.subdomain.cmp(&b.subdomain));
    let file = FileFormatRef {
        version: FILE_VERSION,
        entries,
    };
    // Plain structs of strings and integers: there is no way for this to fail.
    serde_json::to_vec(&file).expect("serialising the blocklist cannot fail")
}

/// Parses and validates a saved blocklist. A malformed file is a version mismatch or an
/// editing mistake, and must stop startup rather than drop suspensions.
fn parse_file(text: &str, path: &Path) -> Result<Vec<Entry>> {
    let file: FileFormat = serde_json::from_str(text)
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

    Ok(entries
        .into_iter()
        .map(|mut entry| {
            entry.reports.truncate(REPORTS_PER_ENTRY);
            entry
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_report() -> Report {
        Report {
            at: 1_700_000_000,
            category: Category::Malware,
            network: "203.0.113.0/24".to_string(),
            reason: Some("Beacon for a miner dropper".to_string()),
        }
    }

    async fn load(dir: &TempDir) -> AbuseList {
        AbuseList::load(dir.path(), Settings::default())
            .await
            .unwrap()
    }

    fn label(n: usize) -> String {
        format!("{n:08}")
    }

    fn entry(subdomain: &str, source: Source, updated_at: i64) -> Entry {
        Entry {
            subdomain: subdomain.to_string(),
            category: Category::Other,
            source,
            suspended_at: updated_at,
            updated_at,
            report_count: 0,
            reports: Vec::new(),
            admin_note: None,
        }
    }

    fn write_entries(dir: &TempDir, entries: &[Entry]) {
        let file = FileFormatRef {
            version: FILE_VERSION,
            entries: entries.iter().collect(),
        };
        std::fs::write(
            dir.path().join("abuse.json"),
            serde_json::to_vec(&file).unwrap(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn starts_empty_when_nothing_was_saved() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir).await;

        assert!(!list.is_blocked("abcdefgh"));
        assert_eq!(list.blocked_count(), 0);
    }

    #[tokio::test]
    async fn a_report_suspends_at_once_and_persists() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir).await;

        let applied = list.report("abcdefgh", sample_report()).await.unwrap();
        assert!(applied.value);
        assert!(applied.persisted);
        assert!(list.is_blocked("abcdefgh"));

        // A restart must not bring the subdomain back
        let reloaded = load(&dir).await;
        let entry = reloaded.suspension("abcdefgh").unwrap();
        assert_eq!(entry.source, Source::Report);
        assert_eq!(
            entry.reports[0].reason.as_deref(),
            Some("Beacon for a miner dropper")
        );
    }

    #[tokio::test]
    async fn repeated_reports_count_up_and_keep_the_newest() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir).await;

        for n in 0..10 {
            let mut report = sample_report();
            report.reason = Some(format!("report {n}"));
            let applied = list.report("abcdefgh", report).await.unwrap();
            assert_eq!(applied.value, n == 0);
        }

        let entry = list.suspension("abcdefgh").unwrap();
        assert_eq!(entry.report_count, 10);
        assert_eq!(entry.reports.len(), REPORTS_PER_ENTRY);
        // Most recent first
        assert_eq!(entry.reports[0].reason.as_deref(), Some("report 9"));
    }

    #[tokio::test]
    async fn lift_removes_the_suspension_everywhere() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir).await;
        list.report("abcdefgh", sample_report()).await.unwrap();

        assert!(list.lift("abcdefgh").await.value);
        assert!(!list.is_blocked("abcdefgh"));
        assert!(!list.lift("abcdefgh").await.value);
        assert!(!load(&dir).await.is_blocked("abcdefgh"));

        // A new report suspends it again
        assert!(
            list.report("abcdefgh", sample_report())
                .await
                .unwrap()
                .value
        );
    }

    #[tokio::test]
    async fn admin_suspends_and_can_update_the_reason() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir).await;

        let applied = list
            .suspend(
                "abcdefgh",
                Some(Category::Phishing),
                Some("Seen in a phishing kit".to_string()),
            )
            .await
            .unwrap();
        assert!(applied.value);

        let entry = load(&dir).await.suspension("abcdefgh").unwrap();
        assert_eq!(entry.source, Source::Admin);
        assert_eq!(entry.category, Category::Phishing);
        assert_eq!(entry.admin_note.as_deref(), Some("Seen in a phishing kit"));

        // Suspending again changes nothing but the reason
        let again = list
            .suspend("abcdefgh", Some(Category::Malware), None)
            .await
            .unwrap();
        assert!(!again.value);
        let entry = list.suspension("abcdefgh").unwrap();
        assert_eq!(entry.category, Category::Malware);
        assert_eq!(entry.admin_note.as_deref(), Some("Seen in a phishing kit"));

        // An admin confirming a reported suspension makes it the admin's
        list.report("reported", sample_report()).await.unwrap();
        list.suspend("reported", None, None).await.unwrap();
        assert_eq!(list.suspension("reported").unwrap().source, Source::Admin);
    }

    #[tokio::test]
    async fn a_full_list_lifts_the_oldest_reported_suspension_never_an_admins() {
        let dir = TempDir::new().unwrap();
        let mut entries: Vec<Entry> = (0..MAX_ENTRIES - 2)
            .map(|n| entry(&label(n), Source::Admin, 0))
            .collect();
        entries.push(entry("reported", Source::Report, 10));
        entries.push(entry("oldrepor", Source::Report, 5));
        write_entries(&dir, &entries);
        let list = load(&dir).await;

        list.report("newcomer", sample_report()).await.unwrap();
        assert!(list.is_blocked("newcomer"));
        assert!(!list.is_blocked("oldrepor"), "the oldest one goes");
        assert!(list.is_blocked("reported"));

        // Promote the remaining reported ones to admin suspensions: nothing can go now
        list.suspend("reported", None, None).await.unwrap();
        list.suspend("newcomer", None, None).await.unwrap();
        assert_eq!(
            list.report("zzzzzzzz", sample_report()).await.unwrap_err(),
            MutateError::Full
        );
        // Reporting a subdomain that is already suspended still works
        assert!(list.report(&label(0), sample_report()).await.is_ok());
    }

    #[tokio::test]
    async fn concurrent_reports_are_all_kept() {
        let dir = TempDir::new().unwrap();
        let list = Arc::new(load(&dir).await);

        let mut tasks = Vec::new();
        for n in 0..64 {
            let list = list.clone();
            tasks.push(tokio::spawn(async move {
                list.report(&label(n), sample_report()).await.unwrap()
            }));
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

        let applied = list.report("abcdefgh", sample_report()).await.unwrap();
        assert!(!applied.persisted);
        assert!(list.is_blocked("abcdefgh"));
        assert_eq!(list.persist_failures(), 1);

        assert!(!list.lift("abcdefgh").await.persisted);
        assert!(!list.is_blocked("abcdefgh"));
        assert_eq!(list.persist_failures(), 2);
    }

    #[tokio::test]
    async fn verify_admin_distinguishes_unconfigured_from_wrong_token() {
        let dir = TempDir::new().unwrap();

        let open = load(&dir).await;
        assert_eq!(open.verify_admin(None), Err(AdminError::NotConfigured));
        assert_eq!(
            open.verify_admin(Some("secret")),
            Err(AdminError::NotConfigured)
        );

        let closed = AbuseList::load(
            dir.path(),
            Settings {
                admin_token: Some("secret".to_string()),
                ..Settings::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(closed.verify_admin(None), Err(AdminError::Invalid));
        assert_eq!(closed.verify_admin(Some("wrong")), Err(AdminError::Invalid));
        assert_eq!(closed.verify_admin(Some("secret")), Ok(()));
    }

    #[tokio::test]
    async fn corrupt_files_refuse_to_load_and_unreachable_labels_are_skipped() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("abuse.json");
        let entry_json = serde_json::to_string(&entry("abcdefgh", Source::Admin, 0)).unwrap();

        let cases: Vec<(String, &str)> = vec![
            ("not json".to_string(), "not valid JSON"),
            (
                "{\"version\": 2, \"entries\": []}".to_string(),
                "expected 1",
            ),
            (
                format!("{{\"version\": 1, \"entries\": [{entry_json}, {entry_json}]}}"),
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
                "{{\"version\": 1, \"entries\": [{entry_json}, {}]}}",
                entry_json.replace("abcdefgh", "not-a-label")
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
