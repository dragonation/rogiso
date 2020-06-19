use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

pub struct SpinLock {
    flag: AtomicU32,
    next: AtomicU32
}

pub struct SpinLockGuard<'a> {
    lock: &'a SpinLock,
    flag: u32
}

impl<'a> SpinLockGuard<'a> {
    pub fn is_locked(&self) -> bool {
        self.flag != 0
    }
    pub fn unlock(&mut self) {
        self.lock.unlock(self);
        self.flag = 0;
    }
}

impl<'a> Drop for SpinLockGuard<'a> {
    fn drop(&mut self) {
        if self.flag != 0 {
            self.unlock();
        }
    }
}

impl SpinLock {

    #[inline]
    pub fn new() -> SpinLock {
        SpinLock {
            flag: AtomicU32::new(0),
            next: AtomicU32::new(1)
        }
    }

    #[inline]
    pub fn lock(&self) -> SpinLockGuard {

        let flag = self.next.fetch_add(1, Ordering::SeqCst);

        loop {
            if let Ok(_) = self.flag.compare_exchange(0, flag, Ordering::SeqCst, Ordering::SeqCst) {
                break;
            }
        }

        SpinLockGuard {
            lock: self,
            flag: flag
        }

    }

    #[inline]
    pub fn try_lock(&self) -> SpinLockGuard {

        let flag = self.next.fetch_add(1, Ordering::SeqCst);

        if let Ok(_) = self.flag.compare_exchange(0, flag, Ordering::SeqCst, Ordering::SeqCst) {
            SpinLockGuard {
                lock: self,
                flag: flag
            }
        } else {
            SpinLockGuard {
                lock: self,
                flag: 0 
            }
        }

    }

    #[inline]
    fn unlock(&self, guard: &SpinLockGuard) {

        if let Err(_) = self.flag.compare_exchange(guard.flag, 0, Ordering::SeqCst, Ordering::SeqCst) {
            panic!("Invalid spin lock guard key to unlock");
        }

    }

} 

#[test]
fn test_lock() {

    let lock = SpinLock::new();

    {
        let guard = lock.lock();
        assert!(guard.is_locked());
        assert!(!lock.try_lock().is_locked());
    }

    {
        assert!(lock.try_lock().is_locked());
    }

}