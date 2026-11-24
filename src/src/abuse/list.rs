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
const REPORT_RATE_LIMIT: u32 = 3;
const REPORT_RATE_WINDOW: StdDuration = StdDuration::from_secs(60 * 60);
/// How often expired rate-limit buckets are dropped.
const CLEANUP_INTERVAL: StdDuration = StdDuration::from_secs(60);

/// Upper bound on stored entries. The whole file is rewritten on every change, and the
/// report endpoint is anonymous, so growth must have a limit.
const MAX_ENTRIES: usize = 10_000;

/// Reports kept per entry, most recent first. `report_count` holds the true total.
const REPORTS_PER_ENTRY: usize = 5;

const FILE_VERSION: u32 = 1;

/// Why a subdomain was blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Malware,
    Phishing,
    Csam,
    Spam,
    Copyright,
    Other,
}

/// Who blocked the subdomain.
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
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub subdomain: String,
    pub category: Category,
    pub source: Source,
    pub created_at: i64,
    pub report_count: u32,
    #[serde(default)]
    pub reports: Vec<Report>,
    /// Set when an admin blocked the subdomain directly.
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

#[derive(Debug, PartialEq, Eq)]
pub enum MutateError {
    /// The blocklist has reached its entry limit.
    Full,
}

/// Result of a change. `persisted` is false when the change took effect in memory but could
/// not be written to disk, in which case it is lost on restart.
#[derive(Debug)]
pub struct Applied<T> {
    pub value: T,
    pub persisted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportOutcome {
    Blocked,
    AlreadyBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminError {
    /// No admin token is configured, so there is nobody allowed to manage the blocklist.
    NotConfigured,
    Invalid,
}

/// The subdomain blocklist.
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
    reporting_enabled: bool,
    admin_token: Option<String>,
    persist_failures: AtomicU64,
}

impl AbuseList {
    /// Loads the blocklist from `data_dir`.
    ///
    /// Fails when a saved blocklist exists but cannot be read: starting without it would
    /// silently restore every blocked subdomain.
    pub async fn load(
        data_dir: impl AsRef<Path>,
        reporting_enabled: bool,
        admin_token: Option<String>,
    ) -> Result<Self> {
        let storage = Storage::new(data_dir);
        let entries = match storage.load().await? {
            Some(text) => parse_file(&text, storage.path())?,
            None => Vec::new(),
        };
        if let Err(e) = storage.ensure_dir().await {
            error!("Blocked subdomains cannot be saved and will be lost on restart: {e}");
        }

        info!(
            "Loaded {} blocked subdomain(s) from {}",
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
            reporting_enabled,
            admin_token,
            persist_failures: AtomicU64::new(0),
        })
    }

    /// Whether `subdomain` is blocked. Lock-free.
    pub fn is_blocked(&self, subdomain: &str) -> bool {
        self.state.load().entries.contains_key(subdomain)
    }

    /// The blocklist entry for `subdomain`, if it is blocked.
    pub fn entry(&self, subdomain: &str) -> Option<Arc<Entry>> {
        self.state.load().entries.get(subdomain).cloned()
    }

    /// Every entry, in no particular order.
    pub fn entries(&self) -> Vec<Arc<Entry>> {
        self.state.load().entries.values().cloned().collect()
    }

    pub fn blocked_count(&self) -> usize {
        self.state.load().entries.len()
    }

    pub fn reporting_enabled(&self) -> bool {
        self.reporting_enabled
    }

    /// Number of changes since startup that could not be written to disk.
    pub fn persist_failures(&self) -> u64 {
        self.persist_failures.load(Ordering::Relaxed)
    }

    /// Checks the token supplied with an admin request.
    pub fn verify_admin(&self, supplied: Option<&str>) -> Result<(), AdminError> {
        let configured = self
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

    /// Records a report and makes sure `subdomain` is blocked.
    pub async fn report(
        &self,
        subdomain: &str,
        report: Report,
    ) -> Result<Applied<ReportOutcome>, MutateError> {
        self.update(|snapshot| {
            if let Some(existing) = snapshot.entries.get(subdomain) {
                let mut entry = (**existing).clone();
                entry.report_count = entry.report_count.saturating_add(1);
                entry.reports.insert(0, report);
                entry.reports.truncate(REPORTS_PER_ENTRY);
                snapshot
                    .entries
                    .insert(subdomain.to_string(), Arc::new(entry));
                return Some(ReportOutcome::AlreadyBlocked);
            }
            if snapshot.entries.len() >= MAX_ENTRIES {
                return None;
            }
            let entry = Entry {
                subdomain: subdomain.to_string(),
                category: report.category,
                source: Source::Report,
                created_at: report.at,
                report_count: 1,
                reports: vec![report],
                admin_note: None,
            };
            snapshot
                .entries
                .insert(subdomain.to_string(), Arc::new(entry));
            Some(ReportOutcome::Blocked)
        })
        .await
        .ok_or(MutateError::Full)
    }

    /// Blocks `subdomain` as an admin action. The value is false if it already was.
    pub async fn block(
        &self,
        subdomain: &str,
        category: Category,
        note: Option<String>,
    ) -> Result<Applied<bool>, MutateError> {
        self.update(|snapshot| {
            if snapshot.entries.contains_key(subdomain) {
                return Some(false);
            }
            if snapshot.entries.len() >= MAX_ENTRIES {
                return None;
            }
            let entry = Entry {
                subdomain: subdomain.to_string(),
                category,
                source: Source::Admin,
                created_at: get_current_timestamp(),
                report_count: 0,
                reports: Vec::new(),
                admin_note: note,
            };
            snapshot
                .entries
                .insert(subdomain.to_string(), Arc::new(entry));
            Some(true)
        })
        .await
        .ok_or(MutateError::Full)
    }

    /// Lifts the block on `subdomain`. The value is false if it was not blocked.
    pub async fn lift(&self, subdomain: &str) -> Applied<bool> {
        self.update(|snapshot| Some(snapshot.entries.remove(subdomain).is_some()))
            .await
            .expect("lifting a block cannot be refused")
    }

    /// Applies `change` to a copy of the current snapshot, saves it, and publishes it.
    /// A `None` result abandons the change without saving.
    ///
    /// A failed save does not undo the change. It takes effect now and is reported, so a
    /// disk problem can never stop a report from blocking an abusive subdomain.
    async fn update<T>(
        &self,
        change: impl FnOnce(&mut Snapshot) -> Option<T>,
    ) -> Option<Applied<T>> {
        let _writer = self.write_lock.lock().await;

        let mut next = Snapshot {
            entries: self.state.load().entries.clone(),
        };
        let value = change(&mut next)?;

        let contents = encode(&next);
        let persisted = match self.storage.save(&contents).await {
            Ok(()) => true,
            Err(e) => {
                self.persist_failures.fetch_add(1, Ordering::Relaxed);
                error!("Blocked subdomains cannot be saved and will be lost on restart: {e}");
                false
            }
        };
        self.state.store(Arc::new(next));

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
    serde_json::to_vec_pretty(&file).expect("serialising the blocklist cannot fail")
}

/// Parses and validates a saved blocklist. A problem here is a version mismatch or an
/// editing mistake, and must stop startup rather than drop entries.
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

    let mut seen = HashSet::new();
    for entry in &file.entries {
        if !verify_subdomain(
            &entry.subdomain,
            CONFIG.subdomain_length,
            &CONFIG.subdomain_alphabet_set,
        ) {
            return Err(anyhow::anyhow!(
                "Blocklist {} contains '{}', which is not a valid subdomain",
                path.display(),
                entry.subdomain
            ));
        }
        if !seen.insert(entry.subdomain.as_str()) {
            return Err(anyhow::anyhow!(
                "Blocklist {} lists '{}' more than once",
                path.display(),
                entry.subdomain
            ));
        }
        if entry.reports.len() > REPORTS_PER_ENTRY {
            warn!(
                "Blocklist {}: truncating reports of '{}' to {REPORTS_PER_ENTRY}",
                path.display(),
                entry.subdomain
            );
        }
    }

    Ok(file
        .entries
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
            description: "Beacon for a miner dropper".to_string(),
            evidence_url: Some("https://threatfox.abuse.ch/ioc/1982219/".to_string()),
            contact: None,
        }
    }

    async fn load(dir: &TempDir, admin_token: Option<&str>) -> AbuseList {
        AbuseList::load(dir.path(), true, admin_token.map(str::to_string))
            .await
            .unwrap()
    }

    fn label(n: usize) -> String {
        format!("{n:08}")
    }

    #[tokio::test]
    async fn starts_empty_when_nothing_was_saved() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir, None).await;

        assert!(!list.is_blocked("abcdefgh"));
        assert_eq!(list.entries().len(), 0);
    }

    #[tokio::test]
    async fn report_blocks_and_persists_across_loads() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir, None).await;

        let applied = list.report("abcdefgh", sample_report()).await.unwrap();
        assert_eq!(applied.value, ReportOutcome::Blocked);
        assert!(applied.persisted);
        assert!(list.is_blocked("abcdefgh"));
        assert_eq!(list.blocked_count(), 1);

        // A restart must not bring the subdomain back
        let reloaded = load(&dir, None).await;
        assert!(reloaded.is_blocked("abcdefgh"));
        assert_eq!(
            reloaded.entry("abcdefgh").unwrap().reports[0].description,
            "Beacon for a miner dropper"
        );
    }

    #[tokio::test]
    async fn repeated_reports_count_up_and_keep_the_newest() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir, None).await;

        list.report("abcdefgh", sample_report()).await.unwrap();
        for n in 0..10 {
            let mut report = sample_report();
            report.description = format!("report {n}");
            let applied = list.report("abcdefgh", report).await.unwrap();
            assert_eq!(applied.value, ReportOutcome::AlreadyBlocked);
        }

        let entry = list.entry("abcdefgh").unwrap();
        assert_eq!(entry.report_count, 11);
        assert_eq!(entry.reports.len(), REPORTS_PER_ENTRY);
        // Most recent first
        assert_eq!(entry.reports[0].description, "report 9");
        assert_eq!(entry.reports[REPORTS_PER_ENTRY - 1].description, "report 5");

        // The truncation survives a reload, and stays valid
        let reloaded = load(&dir, None).await;
        assert_eq!(
            reloaded.entry("abcdefgh").unwrap().reports.len(),
            REPORTS_PER_ENTRY
        );
    }

    #[tokio::test]
    async fn lift_removes_the_entry_everywhere() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir, None).await;
        list.report("abcdefgh", sample_report()).await.unwrap();

        let applied = list.lift("abcdefgh").await;
        assert!(applied.value);
        assert!(!list.is_blocked("abcdefgh"));
        assert!(!list.lift("abcdefgh").await.value);

        assert!(!load(&dir, None).await.is_blocked("abcdefgh"));
    }

    #[tokio::test]
    async fn admin_block_records_its_origin_and_note() {
        let dir = TempDir::new().unwrap();
        let list = load(&dir, Some("secret")).await;

        let applied = list
            .block(
                "abcdefgh",
                Category::Phishing,
                Some("Seen in a phishing kit".to_string()),
            )
            .await
            .unwrap();
        assert!(applied.value);
        assert!(
            !list
                .block("abcdefgh", Category::Other, None)
                .await
                .unwrap()
                .value,
            "blocking twice changes nothing"
        );

        let entry = load(&dir, Some("secret")).await.entry("abcdefgh").unwrap();
        assert_eq!(entry.source, Source::Admin);
        assert_eq!(entry.category, Category::Phishing);
        assert_eq!(entry.report_count, 0);
        assert_eq!(entry.admin_note.as_deref(), Some("Seen in a phishing kit"));

        // A report on an admin block keeps the origin and counts as a report
        list.report("abcdefgh", sample_report()).await.unwrap();
        let entry = list.entry("abcdefgh").unwrap();
        assert_eq!(entry.source, Source::Admin);
        assert_eq!(entry.report_count, 1);
    }

    #[tokio::test]
    async fn report_is_refused_once_the_entry_limit_is_reached() {
        let dir = TempDir::new().unwrap();
        let entries: Vec<String> = (0..MAX_ENTRIES)
            .map(|n| {
                serde_json::to_string(&Entry {
                    subdomain: label(n),
                    category: Category::Other,
                    source: Source::Admin,
                    created_at: 0,
                    report_count: 0,
                    reports: Vec::new(),
                    admin_note: None,
                })
                .unwrap()
            })
            .collect();
        std::fs::write(
            dir.path().join("abuse.json"),
            format!(r#"{{"version": 1, "entries": [{}]}}"#, entries.join(",")),
        )
        .unwrap();
        let list = load(&dir, None).await;
        assert_eq!(list.blocked_count(), MAX_ENTRIES);

        assert_eq!(
            list.report("zzzzzzzz", sample_report()).await.unwrap_err(),
            MutateError::Full
        );
        assert_eq!(
            list.block("yyyyyyyy", Category::Other, None)
                .await
                .unwrap_err(),
            MutateError::Full
        );
        assert!(!list.is_blocked("zzzzzzzz"));

        // Reporting a subdomain that is already blocked still works
        assert_eq!(
            list.report(&label(0), sample_report()).await.unwrap().value,
            ReportOutcome::AlreadyBlocked
        );
    }

    #[tokio::test]
    async fn concurrent_reports_are_all_kept() {
        let dir = TempDir::new().unwrap();
        let list = Arc::new(load(&dir, None).await);

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
        assert_eq!(load(&dir, None).await.blocked_count(), 64);
    }

    #[tokio::test]
    async fn failed_save_still_blocks_and_is_counted() {
        let dir = TempDir::new().unwrap();
        // A directory where the temp file would be created makes every save fail
        std::fs::create_dir(dir.path().join("abuse.json.tmp")).unwrap();
        let list = load(&dir, None).await;

        let applied = list.report("abcdefgh", sample_report()).await.unwrap();
        assert!(!applied.persisted);
        assert!(list.is_blocked("abcdefgh"));
        assert_eq!(list.persist_failures(), 1);

        // Lifting goes through the same path
        assert!(!list.lift("abcdefgh").await.persisted);
        assert!(!list.is_blocked("abcdefgh"));
        assert_eq!(list.persist_failures(), 2);
    }

    #[tokio::test]
    async fn verify_admin_distinguishes_unconfigured_from_wrong_token() {
        let dir = TempDir::new().unwrap();

        let open = load(&dir, None).await;
        assert_eq!(open.verify_admin(None), Err(AdminError::NotConfigured));
        assert_eq!(
            open.verify_admin(Some("secret")),
            Err(AdminError::NotConfigured)
        );

        let closed = load(&dir, Some("secret")).await;
        assert_eq!(closed.verify_admin(None), Err(AdminError::Invalid));
        assert_eq!(closed.verify_admin(Some("wrong")), Err(AdminError::Invalid));
        assert_eq!(closed.verify_admin(Some("secret")), Ok(()));
    }

    #[tokio::test]
    async fn corrupt_or_inconsistent_files_refuse_to_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("abuse.json");
        let entry = Entry {
            subdomain: "abcdefgh".to_string(),
            category: Category::Other,
            source: Source::Admin,
            created_at: 0,
            report_count: 0,
            reports: Vec::new(),
            admin_note: None,
        };
        let entry_json = serde_json::to_string(&entry).unwrap();

        let cases: Vec<(String, &str)> = vec![
            ("not json".to_string(), "not valid JSON"),
            (
                "{\"version\": 2, \"entries\": []}".to_string(),
                "expected 1",
            ),
            (
                format!(
                    "{{\"version\": 1, \"entries\": [{}, {}]}}",
                    entry_json, entry_json
                ),
                "more than once",
            ),
            (
                format!(
                    "{{\"version\": 1, \"entries\": [{}]}}",
                    entry_json.replace("abcdefgh", "not-a-label")
                ),
                "not a valid subdomain",
            ),
        ];
        for (contents, expected) in cases {
            std::fs::write(&path, &contents).unwrap();
            let err = AbuseList::load(dir.path(), true, None).await.err().unwrap();
            assert!(err.to_string().contains(expected), "{contents}: {err}");
        }

        // An entry list is optional
        std::fs::write(&path, r#"{"version": 1}"#).unwrap();
        assert_eq!(load(&dir, None).await.blocked_count(), 0);
    }
}
