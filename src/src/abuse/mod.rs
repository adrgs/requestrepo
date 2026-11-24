//! Subdomain blocklist.
//!
//! A blocked subdomain is dead everywhere: its traffic is refused before it is logged or
//! served, its owner's token stops working, and its name stops resolving. Entries come from
//! public abuse reports or from an admin, and are saved to disk so a restart cannot quietly
//! bring an abusive subdomain back.

pub(crate) mod limiter;
mod list;
pub(crate) mod routes;
mod storage;

pub(crate) use list::{
    AbuseList, AdminError, Applied, Category, Entry, MutateError, Report, ReportOutcome, Source,
};
