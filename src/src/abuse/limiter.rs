use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Buckets tracked at once. One is kept per network that reported inside the current
/// window; beyond this, new networks are refused instead of memory growing without bound.
const MAX_BUCKETS: usize = 100_000;

/// Fixed-window rate limiter keyed by client network.
pub struct ReportLimiter {
    buckets: Mutex<HashMap<IpAddr, Bucket>>,
    max: u32,
    window: Duration,
}

#[derive(Clone, Copy)]
struct Bucket {
    window_start: Instant,
    count: u32,
}

impl ReportLimiter {
    pub fn new(max: u32, window: Duration) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            max,
            window,
        }
    }

    /// Records one attempt from `ip`. On failure, returns how long until its window resets.
    pub fn check(&self, ip: IpAddr) -> Result<(), Duration> {
        self.check_at(ip, Instant::now())
    }

    /// Check and increment in a single step, so concurrent requests cannot each read the
    /// same count and all be let through. `now` is a parameter so that the window logic
    /// can be tested without waiting for a window to pass.
    fn check_at(&self, ip: IpAddr, now: Instant) -> Result<(), Duration> {
        let key = bucket_key(ip);
        let mut buckets = self.lock();

        if let Some(bucket) = buckets.get_mut(&key) {
            let elapsed = now.saturating_duration_since(bucket.window_start);
            if elapsed >= self.window {
                bucket.window_start = now;
                bucket.count = 1;
                return Ok(());
            }
            if bucket.count >= self.max {
                return Err(self.window - elapsed);
            }
            bucket.count += 1;
            return Ok(());
        }

        if buckets.len() >= MAX_BUCKETS {
            // Full: reclaim what has expired, and refuse new networks if it still is.
            Self::drop_expired(&mut buckets, now, self.window);
            if buckets.len() >= MAX_BUCKETS {
                return Err(self.window);
            }
        }
        buckets.insert(
            key,
            Bucket {
                window_start: now,
                count: 1,
            },
        );
        Ok(())
    }

    /// Forgets buckets whose window has ended.
    pub fn sweep(&self) {
        Self::drop_expired(&mut self.lock(), Instant::now(), self.window);
    }

    fn drop_expired(buckets: &mut HashMap<IpAddr, Bucket>, now: Instant, window: Duration) {
        buckets.retain(|_, bucket| now.saturating_duration_since(bucket.window_start) < window);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<IpAddr, Bucket>> {
        // A panic while holding the lock leaves nothing half-updated that matters: a
        // bucket holds only a count, so recover the map rather than poison every request.
        self.buckets.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The address two clients share a budget under: an IPv4 address, or an IPv6 /64. An IPv6
/// allocation hands out 2^64 addresses, so counting per address would be no limit at all.
fn bucket_key(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(mapped) => IpAddr::V4(mapped),
            None => {
                let [a, b, c, d, ..] = v6.segments();
                IpAddr::V6(Ipv6Addr::new(a, b, c, d, 0, 0, 0, 0))
            }
        },
    }
}

/// The reporter's network in CIDR notation (`203.0.113.0/24` or `2001:db8::/64`). Reports
/// keep this instead of the exact address: it is enough to tell reporters apart.
pub fn network_label(ip: IpAddr) -> String {
    match bucket_key(ip) {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            format!("{a}.{b}.{c}.0/24")
        }
        IpAddr::V6(v6) => format!("{v6}/64"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const WINDOW: Duration = Duration::from_secs(3600);
    const V4: IpAddr = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7));
    const V6: IpAddr = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 1, 0, 0, 0, 1));

    /// An address in `2001:db8:0:<network>::/64`.
    fn ipv6(network: u16, host: u16) -> IpAddr {
        IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, network, 0, 0, 0, host))
    }

    #[test]
    fn allows_up_to_the_limit_then_refuses() {
        let limiter = ReportLimiter::new(3, WINDOW);
        let now = Instant::now();

        assert_eq!(limiter.check_at(V4, now), Ok(()));
        assert_eq!(limiter.check_at(V4, now), Ok(()));
        assert_eq!(limiter.check_at(V4, now), Ok(()));
        assert_eq!(limiter.check_at(V4, now), Err(WINDOW));
        // An hour later the budget is available again
        assert_eq!(limiter.check_at(V4, now + WINDOW), Ok(()));
    }

    #[test]
    fn retry_after_counts_down() {
        let limiter = ReportLimiter::new(1, WINDOW);
        let now = Instant::now();
        limiter.check_at(V4, now).unwrap();

        let later = now + Duration::from_secs(600);
        assert_eq!(
            limiter.check_at(V4, later),
            Err(WINDOW - Duration::from_secs(600))
        );
    }

    #[test]
    fn networks_have_separate_budgets() {
        let limiter = ReportLimiter::new(1, WINDOW);
        let now = Instant::now();

        assert_eq!(limiter.check_at(V4, now), Ok(()));
        assert_eq!(limiter.check_at(V6, now), Ok(()));
    }

    #[test]
    fn ipv6_shares_a_budget_per_64() {
        let limiter = ReportLimiter::new(1, WINDOW);
        let now = Instant::now();

        assert_eq!(limiter.check_at(ipv6(1, 1), now), Ok(()));
        assert_eq!(limiter.check_at(ipv6(1, 2), now), Err(WINDOW));
        // A neighbouring /64 is a different network
        assert_eq!(limiter.check_at(ipv6(2, 1), now), Ok(()));
    }

    #[test]
    fn ipv4_mapped_ipv6_counts_as_the_ipv4_address() {
        let limiter = ReportLimiter::new(1, WINDOW);
        let now = Instant::now();
        let mapped = IpAddr::V6(Ipv4Addr::new(198, 51, 100, 7).to_ipv6_mapped());

        assert_eq!(limiter.check_at(mapped, now), Ok(()));
        assert_eq!(limiter.check_at(V4, now), Err(WINDOW));
    }

    #[test]
    fn network_label_widens_the_address() {
        assert_eq!(network_label(V4), "198.51.100.0/24");
        assert_eq!(network_label(ipv6(1, 2)), "2001:db8:0:1::/64");
        assert_eq!(
            network_label(IpAddr::V6(Ipv4Addr::new(198, 51, 100, 7).to_ipv6_mapped())),
            "198.51.100.0/24"
        );
    }

    #[test]
    fn sweep_forgets_expired_buckets() {
        let limiter = ReportLimiter::new(1, WINDOW);
        let now = Instant::now();
        limiter.check_at(V4, now).unwrap();

        // Not yet expired: still refused
        limiter.sweep();
        assert_eq!(
            limiter.check_at(V4, now + WINDOW - Duration::from_secs(1)),
            Err(Duration::from_secs(1))
        );

        // Expired buckets are dropped, freeing the memory they held
        limiter.check_at(V4, now + 2 * WINDOW).unwrap();
        limiter.sweep();
        assert_eq!(limiter.lock().len(), 1);
    }

    #[test]
    fn check_uses_the_current_time() {
        let limiter = ReportLimiter::new(1, WINDOW);
        assert_eq!(limiter.check(V4), Ok(()));

        // A moment has passed, so slightly less than a full window remains
        let retry_after = limiter.check(V4).unwrap_err();
        assert!(retry_after <= WINDOW && retry_after > WINDOW - Duration::from_secs(1));
    }

    #[test]
    fn refuses_new_networks_when_full_until_buckets_expire() {
        let limiter = ReportLimiter::new(1, WINDOW);
        let now = Instant::now();

        for n in 0..MAX_BUCKETS {
            let ip = IpAddr::V4(Ipv4Addr::from(n as u32));
            assert_eq!(limiter.check_at(ip, now), Ok(()));
        }
        // Networks already known are still limited as usual, but nothing new is tracked
        let newcomer = |host: u8| IpAddr::V4(Ipv4Addr::new(203, 0, 113, host));
        assert_eq!(limiter.check_at(newcomer(1), now), Err(WINDOW));

        // Once the old buckets have expired there is room again, and the pressure of the
        // newcomer reclaims all of them at once
        assert_eq!(limiter.check_at(newcomer(2), now + WINDOW), Ok(()));
        assert_eq!(limiter.lock().len(), 1);
    }
}
