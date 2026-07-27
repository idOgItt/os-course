use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicIsize, AtomicBool, Ordering},
};

pub struct RwLock<T> {
    data: UnsafeCell<T>,
    state: AtomicIsize,
    writer_waiting: AtomicBool
}

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            state: AtomicIsize::new(0),
            writer_waiting: AtomicBool::new(false),
        }
    }

    pub fn read(&self) -> ReadGuard<'_, T> {
        loop {
            while self.writer_waiting.load(Ordering::Acquire) || self.state.load(Ordering::Acquire) < 0 {
                core::hint::spin_loop();
            }

            let current_state = self.state.load(Ordering::Acquire);
            if current_state >= 0 {
                if self.state
                    .compare_exchange(current_state, current_state + 1, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            }
            core::hint::spin_loop();
        }
        ReadGuard { rwlock: self }
    }

    pub fn write(&self) -> WriteGuard<'_, T> {
        self.writer_waiting.store(true, Ordering::Release);

        loop {
            if self.state.compare_exchange(0, -1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
                break;
            }
            core::hint::spin_loop();
        }

        self.writer_waiting.store(false, Ordering::Release);
        WriteGuard { rwlock: self }
    }
}

unsafe impl<T: Send> Send for RwLock<T> {}
unsafe impl<T: Send> Sync for RwLock<T> {}

pub struct ReadGuard<'a, T> {
    rwlock: &'a RwLock<T>,
}

impl<T> Deref for ReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.data.get() }
    }
}

impl<T> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        self.rwlock.state.fetch_sub(1, Ordering::Release);
    }
}

pub struct WriteGuard<'a, T> {
    rwlock: &'a RwLock<T>,
}

impl<T> Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.rwlock.data.get() }
    }
}

impl<T> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.rwlock.data.get() }
    }
}

impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.rwlock.state.store(0, Ordering::Release);
    }
}
