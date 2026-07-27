use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

// TODO: your code here.


pub struct RwLock<T> {
    // TODO: your code here.
    data: UnsafeCell<T>,
}


impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            // TODO: your code here.
            data: UnsafeCell::new(data),
        }
    }


    pub fn read(&self) -> ReadGuard<'_, T> {
        // TODO: your code here.
        unimplemented!();
    }


    pub fn write(&self) -> WriteGuard<'_, T> {
        // TODO: your code here.
        unimplemented!();
    }
}


unsafe impl<T: Send> Send for RwLock<T> {
}


unsafe impl<T: Send> Sync for RwLock<T> {
}


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
        // TODO: your code here.
        unimplemented!();
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
        // TODO: your code here.
        unimplemented!();
    }
}


// TODO: your code here.
