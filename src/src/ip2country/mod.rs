use anyhow::Result;
use flate2::read::GzDecoder;
use lazy_static::lazy_static;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;
use std::sync::RwLock;
use tracing::{info, warn};

/// IP range entry storing start IP and country code
/// Uses DB-IP format: we only store the start IP and country
/// and use binary search to find the matching range
#[derive(Debug, Clone)]
struct IpEntry {
    start: u32,
    country: String,
}

lazy_static! {
    static ref IP_ENTRIES: RwLock<Vec<IpEntry>> = RwLock::new(Vec::new());
    static ref SHOW_COUNTRY: RwLock<bool> = RwLock::new(false);
}

pub fn init() -> Result<()> {
    if !IP_ENTRIES.read().unwrap().is_empty() {
        return Ok(());
    }

    // Try multiple possible paths for the database
    let paths = [
        "ip2country/vendor/dbip-country-lite.csv.gz",
        "../ip2country/vendor/dbip-country-lite.csv.gz",
        "dbip-country-lite.csv.gz",
    ];

    let csv_path = paths.iter().find(|p| Path::new(p).exists());

    let csv_path = match csv_path {
        Some(path) => Path::new(path),
        None => {
            warn!("IP2Country database not found. Country lookup disabled.");
            warn!("Download from: https://db-ip.com/db/download/ip-to-country-lite");
            warn!("Place at: ip2country/vendor/dbip-country-lite.csv.gz");
            return Ok(());
        }
    };

    info!("Loading IP2Country database from {}", csv_path.display());

    let file = File::open(csv_path)?;
    let decoder = GzDecoder::new(file);
    let reader = BufReader::new(decoder);

    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            // DB-IP format: start_ip,end_ip,country_code
            let start_ip = parts[0];
            let country = parts[2].trim().to_string();

            // Only process IPv4 addresses
            if let Some(start) = ip_to_u32(start_ip) {
                entries.push(IpEntry { start, country });
            }
        }
    }

    entries.sort_by_key(|e| e.start);

    let count = entries.len();
    let mut db = IP_ENTRIES.write().unwrap();
    *db = entries;

    *SHOW_COUNTRY.write().unwrap() = true;

    info!("Loaded {} IP ranges", count);

    Ok(())
}

fn ip_to_u32(ip: &str) -> Option<u32> {
    Ipv4Addr::from_str(ip).ok().map(u32::from)
}

/// Check if an IP string is a valid IPv4 address
fn is_ipv4(ip: &str) -> bool {
    ip.split('.').filter_map(|p| p.parse::<u8>().ok()).count() == 4
}

/// Look up the country code for an IP address
/// Returns None if IP is invalid or database not loaded
pub fn lookup_country(ip: &str) -> Option<String> {
    // Check if country lookup is enabled
    if !*SHOW_COUNTRY.read().ok()? {
        return None;
    }

    // Validate and convert IP
    if !is_ipv4(ip) {
        return None;
    }

    let ip_num = ip_to_u32(ip)?;

    let db = IP_ENTRIES.read().ok()?;
    if db.is_empty() {
        return None;
    }

    // Binary search to find the entry where start <= ip_num
    // We want the rightmost entry where start <= ip_num
    let idx = db.partition_point(|e| e.start <= ip_num);

    if idx == 0 {
        return None;
    }

    // The country is in the entry just before the partition point
    Some(db[idx - 1].country.clone())
}
