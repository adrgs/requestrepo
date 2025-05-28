
use anyhow::Result;
use flate2::read::GzDecoder;
use lazy_static::lazy_static;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;
use std::sync::RwLock;
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
struct IpRange {
    start: u32,
    end: u32,
    country: String,
}

lazy_static! {
    static ref IP_RANGES: RwLock<Vec<IpRange>> = RwLock::new(Vec::new());
}

pub fn init() -> Result<()> {
    if !IP_RANGES.read().unwrap().is_empty() {
        return Ok(());
    }

    let csv_path = Path::new("ip2country/ip2country.csv.gz");
    if !csv_path.exists() {
        error!("IP2Country database not found at {}", csv_path.display());
        return Ok(());
    }

    info!("Loading IP2Country database from {}", csv_path.display());

    let file = File::open(csv_path)?;
    let decoder = GzDecoder::new(file);
    let reader = BufReader::new(decoder);

    let mut ranges = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            let start = u32::from_str(parts[0])?;
            let end = u32::from_str(parts[1])?;
            let country = parts[2].to_string();
            ranges.push(IpRange { start, end, country });
        }
    }

    ranges.sort_by_key(|range| range.start);

    let mut db = IP_RANGES.write().unwrap();
    *db = ranges;

    info!("Loaded {} IP ranges", db.len());

    Ok(())
}

fn ip_to_u32(ip: &str) -> Option<u32> {
    Ipv4Addr::from_str(ip).ok().map(|ip| u32::from(ip))
}

pub fn lookup_country(ip: &str) -> Option<String> {
    let ip_num = match ip_to_u32(ip) {
        Some(num) => num,
        None => return None,
    };

    let db = match IP_RANGES.read() {
        Ok(db) => db,
        Err(_) => return None,
    };

    let mut left = 0;
    let mut right = db.len();

    while left < right {
        let mid = left + (right - left) / 2;
        let range = &db[mid];

        if ip_num < range.start {
            right = mid;
        } else if ip_num > range.end {
            left = mid + 1;
        } else {
            return Some(range.country.clone());
        }
    }

    None
}
