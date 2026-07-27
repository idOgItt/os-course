#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use core::cell::UnsafeCell;


#[repr(C)]
pub struct Atomic(UnsafeCell<usize>);


impl Atomic {
    pub const fn new(value: usize) -> Self {
        Self(UnsafeCell::new(value))
    }


    pub fn load(&self) -> usize {
        unsafe { atomic_load(self.0.get()) }
    }


    pub fn store(&self, value: usize) {
        unsafe {
            atomic_store(self.0.get(), value);
        }
    }


    pub fn add(&self, value: usize) {
        unsafe {
            atomic_add(self.0.get(), value);
        }
    }


    pub fn sub(&self, value: usize) {
        unsafe {
            atomic_sub(self.0.get(), value);
        }
    }


    pub fn exchange(&self, value: usize) -> usize {
        unsafe { atomic_xchg(self.0.get(), value) }
    }


    pub fn compare_exchange(&self, old_value: &mut usize, new_value: usize) -> bool {
        unsafe { atomic_cmpxchg(self.0.get(), old_value as *mut usize, new_value) }
    }
}


unsafe impl Send for Atomic {
}


unsafe impl Sync for Atomic {
}


unsafe extern "C" {
    /// Атомарно читает по указателю `atomic`.
    fn atomic_load(atomic: *const usize) -> usize;


    /// Атомарно записывает `value` по указателю `atomic`.
    fn atomic_store(atomic: *mut usize, value: usize);


    /// Атомарно прибавляет число `value` к значению по указателю `atomic`.
    fn atomic_add(atomic: *mut usize, value: usize);


    /// Атомарно вычитает число `value` из значения по указателю `atomic`.
    fn atomic_sub(atomic: *mut usize, value: usize);


    /// Атомарно заменяет значение по указателю `atomic` на `value` и возвращает старое значение.
    fn atomic_xchg(atomic: *mut usize, value: usize) -> usize;


    /// Атомарно выполняет следующее действие.
    /// Сравнивает значение по указателю `atomic` с `old_value`.
    /// Если они совпадают, заменяет `atomic` на `new_value` и возвращает `true`.
    /// Если они отличаются, записывает значение `atomic` в `old_value` и возвращает `false`.
    fn atomic_cmpxchg(atomic: *mut usize, old_value: *mut usize, new_value: usize) -> bool;
}
