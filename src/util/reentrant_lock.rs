use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

pub struct ReentrantToken {
    lock: Arc<ReentrantLock>,
    reading: AtomicU32,
    reading_flag: u32,
    writing: AtomicU32,
    writing_flag: u32
}

impl ReentrantToken {

    pub fn new(lock: Arc<ReentrantLock>) -> ReentrantToken {

        let reading_flag = lock.gain_flag();
        let writing_flag = lock.gain_flag();

        ReentrantToken {
            lock: lock,
            reading: AtomicU32::new(0),
            reading_flag: reading_flag,
            writing: AtomicU32::new(0),
            writing_flag: writing_flag,
        }

    }

    pub fn lock_read<'a>(&'a self) -> ReentrantLockReadGuard<'a> {

        self.lock.lock_read(self)

    }

    pub fn try_lock_read<'a>(&'a self) -> ReentrantLockReadGuard<'a> {

        self.lock.try_lock_read(self)

    }

    pub fn unlock_read(&self) {

        self.lock.unlock_read(self);

    }

    pub fn lock_write<'a>(&'a self) -> ReentrantLockWriteGuard<'a> {

        self.lock.lock_write(self)

    }

    pub fn try_lock_write<'a>(&'a self) -> ReentrantLockWriteGuard<'a> {

        self.lock.try_lock_write(self)

    }

    pub fn unlock_write(&self) {

        self.lock.unlock_write(self);

    }

}

pub struct ReentrantLock {
    reading: AtomicU32,
    flag: AtomicU32,
    next: AtomicU32
}

pub struct ReentrantLockReadGuard<'a> {
    token: &'a ReentrantToken,
    locked: bool 
}

impl<'a> ReentrantLockReadGuard<'a> {
    pub fn is_locked(&self) -> bool {
        self.locked
    }
    pub fn unlock(&mut self) {
        if self.locked {
            self.token.unlock_read();
            self.locked = false;
        }
    }
}

impl<'a> Drop for ReentrantLockReadGuard<'a> {
    fn drop(&mut self) {
        self.unlock();
    }
}

pub struct ReentrantLockWriteGuard<'a> {
    token: &'a ReentrantToken,
    locked: bool 
}

impl<'a> ReentrantLockWriteGuard<'a> {
    pub fn is_locked(&self) -> bool {
        self.locked
    }
    pub fn unlock(&mut self) {
        if self.locked {
            self.locked = false;
            self.token.unlock_write();
        }
    }
}

impl<'a> Drop for ReentrantLockWriteGuard<'a> {
    fn drop(&mut self) {
        self.unlock();
    }
}

impl ReentrantLock {

    #[inline]
    pub fn new() -> ReentrantLock {
        ReentrantLock {
            reading: AtomicU32::new(0),
            flag: AtomicU32::new(0),
            next: AtomicU32::new(1)
        }
    }

    pub fn gain_flag(&self) -> u32 {

        self.next.fetch_add(1, Ordering::SeqCst)

    }

    #[inline]
    pub fn lock_read<'a>(&self, token: &'a ReentrantToken) -> ReentrantLockReadGuard<'a> {

        let flag = token.reading_flag;
        if (token.reading.load(Ordering::SeqCst) > 0) || (token.writing.load(Ordering::SeqCst) > 0) {
            self.reading.fetch_add(1, Ordering::SeqCst);
            token.reading.fetch_add(1, Ordering::SeqCst);
            return ReentrantLockReadGuard {
                token: token,
                locked: true
            };
        }

        loop {
            if let Ok(_) = self.flag.compare_exchange(0, flag, Ordering::SeqCst, Ordering::SeqCst) {
                break;
            }
        }
        self.reading.fetch_add(1, Ordering::SeqCst);
        token.reading.fetch_add(1, Ordering::SeqCst);

        if let Err(_) = self.flag.compare_exchange(flag, 0, Ordering::SeqCst, Ordering::SeqCst) {
            panic!("Invalid reentrant lock read guard to unlock");
        }

        ReentrantLockReadGuard {
            token: token,
            locked: true
        }

    }

    #[inline]
    pub fn try_lock_read<'a>(&self, token: &'a ReentrantToken) -> ReentrantLockReadGuard<'a> {

        let flag = token.reading_flag;
        if (token.reading.load(Ordering::SeqCst) > 0) || (token.writing.load(Ordering::SeqCst) > 0) {
            self.reading.fetch_add(1, Ordering::SeqCst);
            token.reading.fetch_add(1, Ordering::SeqCst);
            return ReentrantLockReadGuard {
                token: token,
                locked: true
            };
        }

        if let Ok(_) = self.flag.compare_exchange(0, flag, Ordering::SeqCst, Ordering::SeqCst) {

            self.reading.fetch_add(1, Ordering::SeqCst);
            token.reading.fetch_add(1, Ordering::SeqCst);

            if let Err(_) = self.flag.compare_exchange(flag, 0, Ordering::SeqCst, Ordering::SeqCst) {
                panic!("Invalid reentrant lock read guard to unlock");
            }

            ReentrantLockReadGuard {
                token: token,
                locked: true
            }

        } else {

            ReentrantLockReadGuard {
                token: token,
                locked: false
            }

        }

    }

    #[inline]
    fn unlock_read(&self, token: &ReentrantToken) {

        self.reading.fetch_sub(1, Ordering::SeqCst);
        token.reading.fetch_sub(1, Ordering::SeqCst);

    }

    #[inline]
    pub fn lock_write<'a>(&self, token: &'a ReentrantToken) -> ReentrantLockWriteGuard<'a> {

        let flag = token.writing_flag;
        if (token.reading.load(Ordering::SeqCst) > 0) && (token.writing.load(Ordering::SeqCst) == 0) {
            panic!("Reentrant lock is locked for reading on the token, but writing expected");
        }

        if token.writing.load(Ordering::SeqCst) > 0 {
            token.writing.fetch_add(1, Ordering::SeqCst);
            return ReentrantLockWriteGuard {
                token: token,
                locked: true
            };
        }

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

        token.writing.fetch_add(1, Ordering::SeqCst);

        ReentrantLockWriteGuard {
            token: token,
            locked: true
        }

    }

    #[inline]
    pub fn try_lock_write<'a>(&self, token: &'a ReentrantToken) -> ReentrantLockWriteGuard<'a> {

        let flag = token.writing_flag;
        if (token.reading.load(Ordering::SeqCst) > 0) && (token.writing.load(Ordering::SeqCst) == 0) {
            return ReentrantLockWriteGuard {
                token: token,
                locked: false
            };
        }

        if token.writing.load(Ordering::SeqCst) > 0 {
            token.writing.fetch_add(1, Ordering::SeqCst);
            return ReentrantLockWriteGuard {
                token: token,
                locked: true
            };
        }

        if let Ok(_) = self.flag.compare_exchange(0, flag, Ordering::SeqCst, Ordering::SeqCst) {
            if self.reading.load(Ordering::SeqCst) == 0 {
                token.writing.fetch_add(1, Ordering::SeqCst);
                return ReentrantLockWriteGuard {
                    token: token,
                    locked: true
                };
            } else {
                if let Err(_) = self.flag.compare_exchange(flag, 0, Ordering::SeqCst, Ordering::SeqCst) {
                    panic!("Reentrant lock is locked by violatile token");
                }
            }
        }

        ReentrantLockWriteGuard {
            token: token,
            locked: false
        }

    }

    #[inline]
    fn unlock_write(&self, token: &ReentrantToken) {

        if (token.writing.load(Ordering::SeqCst) == 1) && (token.reading.load(Ordering::SeqCst) > 0) {
            panic!("Reentrant lock is unlocked for writing on the token, but reading remains while writing released");
        }

        token.writing.fetch_sub(1, Ordering::SeqCst);
        if token.writing.load(Ordering::SeqCst) == 0 {
            if let Err(_) = self.flag.compare_exchange(token.writing_flag, 0, Ordering::SeqCst, Ordering::SeqCst) {
                panic!("Invalid reentrant lock guard to unlock");
            }
        }

    }

} 

#[test]
fn test_lock() {

    let lock = Arc::new(ReentrantLock::new());

    let token = ReentrantToken::new(lock.clone());
    let token_2 = ReentrantToken::new(lock);

    {
        let guard = token.lock_write();
        let guard_2 = token.try_lock_write();
        let guard_3 = token.try_lock_read();
        assert!(guard.is_locked());
        assert!(guard_2.is_locked());
        assert!(guard_3.is_locked());
    }

    {
        let guard = token.lock_read();
        let guard_2 = token.lock_read();
        let guard_3 = token.try_lock_read();
        let guard_4 = token.try_lock_write();
        assert!(guard.is_locked());
        assert!(guard_2.is_locked());
        assert!(guard_3.is_locked());
        assert!(!guard_4.is_locked());
    }

    {
        let guard = token.lock_write();
        let guard_2 = token_2.try_lock_write();
        let guard_3 = token_2.try_lock_read();
        assert!(guard.is_locked());
        assert!(!guard_2.is_locked());
        assert!(!guard_3.is_locked());
    }

    {
        let guard = token.lock_read();
        let guard_2 = token_2.lock_read();
        let guard_3 = token_2.try_lock_read();
        let guard_4 = token.try_lock_write();
        let guard_5 = token_2.try_lock_write();
        assert!(guard.is_locked());
        assert!(guard_2.is_locked());
        assert!(guard_3.is_locked());
        assert!(!guard_4.is_locked());
        assert!(!guard_5.is_locked());
    }

}