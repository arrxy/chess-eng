use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Paces connection setup to a fixed rate (handshakes/sec), so a run through a
/// load balancer stays under its TLS-handshake limit regardless of how fast the
/// client is. Each `acquire` reserves the next evenly-spaced time slot.
pub struct RateGate {
    interval: Duration,
    next: Mutex<Instant>,
}

impl RateGate {
    pub fn new(per_sec: f64) -> Self {
        let interval = Duration::from_secs_f64(1.0 / per_sec.max(0.000_001));
        Self {
            interval,
            next: Mutex::new(Instant::now()),
        }
    }

    pub async fn acquire(&self) {
        let wait = {
            let mut next = self.next.lock().await;
            let now = Instant::now();
            let slot = (*next).max(now);
            *next = slot + self.interval;
            slot.saturating_duration_since(now)
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
}
