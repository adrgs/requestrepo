//! Subdomain blocklist.
//!
//! Anyone can report a subdomain, and a report suspends it at once: nobody has to be
//! awake for an abusive subdomain to stop working, and the owner of a legitimate one that is
//! caught by mistake can simply create a new session. Reports are rate limited per IP.
//!
//! A suspended subdomain is a sinkhole: nothing is served from it, its name stops
//! resolving, its owner's token stops working, but its incoming HTTP and DNS traffic is
//! still recorded for the admin, because the sources of a botnet beacon are its victims and
//! are worth passing on to whoever can notify them. Suspensions are saved to disk so that a
//! restart cannot quietly bring an abusive subdomain back.

pub(crate) mod limiter;
mod list;
pub(crate) mod routes;
mod storage;

pub(crate) use list::{
    AbuseList, AdminError, Applied, Category, Entry, MutateError, Report, Settings, Source,
    REPORT_RATE_LIMIT, REPORT_RATE_WINDOW,
};
