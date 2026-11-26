//! Subdomain blocklist.
//!
//! Anyone can report a subdomain, and a report suspends it at once: nobody has to be
//! awake for an abusive subdomain to stop working, and the owner of a legitimate one that is
//! caught by mistake can simply create a new session. Reports are rate limited per IP, and
//! on a private instance (ADMIN_TOKEN set) they need the admin token like everything else.
//!
//! A suspended subdomain serves nothing, stops resolving, refuses mail, and its owner's
//! token stops working. Suspensions are saved to `DATA_DIR/abuse.json`, one per line, so a
//! restart cannot bring an abusive subdomain back; deleting a line lifts a suspension.

pub(crate) mod limiter;
mod list;
pub(crate) mod routes;
mod storage;

pub(crate) use list::{AbuseList, Settings, REPORT_RATE_LIMIT, REPORT_RATE_WINDOW};
