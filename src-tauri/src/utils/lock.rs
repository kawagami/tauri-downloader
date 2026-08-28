// 鎖中毒（poisoning）的統一處理。
//
// `lock().unwrap()` 的問題不是它會 panic，而是它讓「某條路徑 panic 過一次」
// 升級成「這個子系統從此永久壞死」：中毒的鎖之後每次 lock 都回 Err，
// DB、設定、任務清單全部一起陪葬，而且使用者沒有任何自救辦法（重開 app 才行）。
//
// 這裡的取捨很直接：被保護的資料結構都是「一次操作寫完就一致」的東西
// （HashMap、Vec、設定 struct、SQLite connection），中間 panic 留下的
// 不一致風險遠低於整個子系統停擺。所以一律 `into_inner()` 取回資料繼續用。

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub trait LockExt<T: ?Sized> {
    /// 取鎖；中毒時取回內層資料繼續用，不 panic。
    fn lock_safe(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> LockExt<T> for Mutex<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub trait RwLockExt<T: ?Sized> {
    fn read_safe(&self) -> RwLockReadGuard<'_, T>;
    fn write_safe(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T: ?Sized> RwLockExt<T> for RwLock<T> {
    fn read_safe(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|e| e.into_inner())
    }
    fn write_safe(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// 中毒後仍拿得到資料 —— 這正是 `lock().unwrap()` 做不到的
    #[test]
    fn poisoned_mutex_still_yields_data() {
        let m = Arc::new(Mutex::new(vec![1, 2, 3]));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("毒化這把鎖");
        })
        .join();
        assert!(m.lock().is_err(), "前置條件：鎖確實中毒了");
        assert_eq!(m.lock_safe().as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn poisoned_rwlock_still_yields_data() {
        let l = Arc::new(RwLock::new(7u32));
        let l2 = l.clone();
        let _ = std::thread::spawn(move || {
            let _g = l2.write().unwrap();
            panic!("毒化這把鎖");
        })
        .join();
        assert_eq!(*l.read_safe(), 7);
        *l.write_safe() = 9;
        assert_eq!(*l.read_safe(), 9);
    }
}
