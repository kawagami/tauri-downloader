// 共用限速器 — 網站下載與直鏈下載共用同一套 token bucket。
// 限速值一律 bytes/s（0 = 不限），放在 Arc<AtomicU64> 裡，設定改了立刻生效。
//
// 併發安全：多個分段各自呼叫 acquire()，超額的部分算成「欠帳」讓該分段自己睡，
// 合計吞吐仍收斂到 limit。睡眠切 250ms 小段，取消旗標能即時中斷。

use crate::utils::lock::LockExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 睡眠切段長度：限速期間仍要能即時回應取消。
const SLEEP_STEP: Duration = Duration::from_millis(250);

struct Bucket {
    /// 剩餘可用 bytes（可為負 = 已透支，下一位呼叫者要等）
    tokens: f64,
    last: Instant,
    /// 上次看到的限速值，改變時重置桶避免舊帳影響
    last_limit: u64,
}

pub struct RateLimiter {
    limit_bps: Arc<AtomicU64>,
    bucket: Mutex<Bucket>,
}

impl RateLimiter {
    pub fn new(limit_bps: Arc<AtomicU64>) -> Arc<Self> {
        Arc::new(Self {
            limit_bps,
            bucket: Mutex::new(Bucket {
                tokens: 0.0,
                last: Instant::now(),
                last_limit: 0,
            }),
        })
    }

    /// 限速值本體（0 = 不限）。設定變更時直接 store 進去即可。
    pub fn limit(&self) -> &Arc<AtomicU64> {
        &self.limit_bps
    }

    pub fn set_limit(&self, bps: u64) {
        self.limit_bps.store(bps, Ordering::Relaxed);
    }

    /// 消耗 `bytes` 的配額，必要時 sleep。`cancelled` 回 true 時提前返回 false。
    pub async fn acquire(&self, bytes: u64, cancelled: impl Fn() -> bool) -> bool {
        let debt = {
            let limit = self.limit_bps.load(Ordering::Relaxed);
            if limit == 0 {
                return !cancelled();
            }
            let mut b = self.bucket.lock_safe();
            let now = Instant::now();
            if b.last_limit != limit {
                // 限速剛改：重新起算，不把舊速率的帳算到新速率頭上
                b.tokens = 0.0;
                b.last = now;
                b.last_limit = limit;
            }
            let elapsed = now.duration_since(b.last).as_secs_f64();
            b.last = now;
            // 補充配額，最多囤 1 秒份（避免閒置後爆量）
            b.tokens = (b.tokens + elapsed * limit as f64).min(limit as f64);
            b.tokens -= bytes as f64;
            if b.tokens < 0.0 {
                Duration::from_secs_f64(-b.tokens / limit as f64)
            } else {
                Duration::ZERO
            }
        };

        let mut remaining = debt;
        while remaining > Duration::ZERO {
            if cancelled() {
                return false;
            }
            let d = remaining.min(SLEEP_STEP);
            tokio::time::sleep(d).await;
            remaining -= d;
        }
        !cancelled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unlimited_returns_immediately() {
        let l = RateLimiter::new(Arc::new(AtomicU64::new(0)));
        let t = Instant::now();
        assert!(l.acquire(10 * 1024 * 1024, || false).await);
        assert!(t.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn limited_sleeps_roughly_proportionally() {
        // 1000 bytes/s，一次要 500 bytes → 桶是空的，約需 0.5 秒
        let l = RateLimiter::new(Arc::new(AtomicU64::new(1000)));
        let t = Instant::now();
        assert!(l.acquire(500, || false).await);
        assert!(t.elapsed() >= Duration::from_millis(400), "{:?}", t.elapsed());
    }

    #[tokio::test]
    async fn cancel_breaks_sleep() {
        let l = RateLimiter::new(Arc::new(AtomicU64::new(10)));
        let t = Instant::now();
        // 要 1000 bytes、限速 10 bytes/s → 本來要睡 100 秒，取消要能馬上跳出
        assert!(!l.acquire(1000, || true).await);
        assert!(t.elapsed() < Duration::from_secs(1));
    }
}
