use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

pub struct RwLock {
    reading: AtomicU32,
    flag: AtomicU32,
    next: AtomicU32
}

pub struct RwLockReadGuard<'a> {
    lock: &'a RwLock,
    flag: u32
}

impl<'a> RwLockReadGuard<'a> {

    pub fn is_locked(&self) -> bool {
        self.flag != 0
    }

    pub fn unlock(&mut self) {
        self.lock.unlock_read();
        self.flag = 0;
    }

}

impl<'a> Drop for RwLockReadGuard<'a> {
    fn drop(&mut self) {
        if self.flag != 0 {
            self.unlock();
        }
    }
}

pub struct RwLockWriteGuard<'a> {
    lock: &'a RwLock,
    flag: u32
}

impl<'a> RwLockWriteGuard<'a> {
    pub fn is_locked(&self) -> bool {
        self.flag != 0
    }
    pub fn unlock(&mut self) {
        self.lock.unlock_write(self);
        self.flag = 0;
    }
}

impl<'a> Drop for RwLockWriteGuard<'a> {
    fn drop(&mut self) {
        if self.flag != 0 {
            self.unlock();
        }
    }
}

impl RwLock {

    #[inline]
    pub fn new() -> RwLock {
        RwLock {
            reading: AtomicU32::new(0),
            flag: AtomicU32::new(0),
            next: AtomicU32::new(1)
        }
    }

    #[inline]
    pub fn lock_read(&self) -> RwLockReadGuard {

        let flag = self.next.fetch_add(1, Ordering::SeqCst);

        loop {
            if let Ok(_) = self.flag.compare_exchange(0, flag, Ordering::SeqCst, Ordering::SeqCst) {
                break;
            }
        }
        self.reading.fetch_add(1, Ordering::SeqCst);

        if let Err(_) = self.flag.compare_exchange(flag, 0, Ordering::SeqCst, Ordering::SeqCst) {
            panic!("Invalid rw lock read guard to unlock");
        }

        RwLockReadGuard {
            lock: self,
            flag: flag
        }

    }

    #[inline]
    pub fn try_lock_read(&self) -> RwLockReadGuard {

        let flag = self.next.fetch_add(1, Ordering::SeqCst);

        if let Ok(_) = self.flag.compare_exchange(0, flag, Ordering::SeqCst, Ordering::SeqCst) {

            self.reading.fetch_add(1, Ordering::SeqCst);

            if let Err(_) = self.flag.compare_exchange(flag, 0, Ordering::SeqCst, Ordering::SeqCst) {
                panic!("Invalid rw lock read guard to unlock");
            }

            RwLockReadGuard {
                lock: self,
                flag: flag
            }

        } else {
            RwLockReadGuard {
                lock: self,
                flag: 0 
            }
        }

    }

    #[inline]
    fn unlock_read(&self) {

        self.reading.fetch_sub(1, Ordering::SeqCst);

    }

    #[inline]
    pub fn lock_write(&self) -> RwLockWriteGuard {

        let flag = self.next.fetch_add(1, Ordering::SeqCst);

        loop {
            if let Ok(_) = self.flag.compare_exchange(0, flag, Ordering::SeqCst, Ordering::SeqCst) {
                break;
            }
        }

        loop {
            if self.reading.load(Ordering::SeqCst) == 0 {
                break;
            }
        }

        RwLockWriteGuard {
            lock: self,
            flag: flag
        }

    }

    #[inline]
    pub fn try_lock_write(&self) -> RwLockWriteGuard {

        let flag = self.next.fetch_add(1, Ordering::SeqCst);

        if let Ok(_) = self.flag.compare_exchange(0, flag, Ordering::SeqCst, Ordering::SeqCst) {

            if self.reading.load(Ordering::SeqCst) == 0 {
                RwLockWriteGuard {
                    lock: self,
                    flag: flag
                }
            } else {
                if let Err(_) = self.flag.compare_exchange(flag, 0, Ordering::SeqCst, Ordering::SeqCst) {
                    panic!("Invalid rw lock guard to unlock");
                }
                RwLockWriteGuard {
                    lock: self,
                    flag: 0 
                }
            }

        } else {
            RwLockWriteGuard {
                lock: self,
                flag: 0 
            }
        }

    }

    #[inline]
    fn unlock_write(&self, guard: &RwLockWriteGuard) {

        if let Err(_) = self.flag.compare_exchange(guard.flag, 0, Ordering::SeqCst, Ordering::SeqCst) {
            panic!("Invalid rw lock guard to unlock");
        }

    }

} 

#[test]
fn test_lock() {

    let lock = RwLock::new();

    {
        let guard = lock.lock_write();
        let guard_2 = lock.try_lock_write();
        let guard_3 = lock.try_lock_read();
        assert!(guard.is_locked());
        assert!(!guard_2.is_locked());
        assert!(!guard_3.is_locked());
    }

    {
        let guard = lock.lock_read();
        let guard_2 = lock.lock_read();
        let guard_3 = lock.try_lock_read();
        let guard_4 = lock.try_lock_write();
        assert!(guard.is_locked());
        assert!(guard_2.is_locked());
        assert!(guard_3.is_locked());
        assert!(!guard_4.is_locked());
    }

}