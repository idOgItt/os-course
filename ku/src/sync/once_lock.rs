use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicU8, Ordering},
};

use crate::error::{Error::InvalidArgument, Result};

// Used in docs.
#[allow(unused)]
use {crate::error::Error, spin::Mutex};


/// Аналогично [std::sync::OnceLock](https://doc.rust-lang.org/nightly/std/sync/struct.OnceLock.html)
/// реализует примитив синхронизации для данных, которые один раз записываются, а потом только читаются.
///
/// Используется, например, для инициализации статических констант,
/// которые не могут быть вычислены на этапе компиляции.
pub struct OnceLock<T> {
    /// Защищаемые данные.
    data: UnsafeCell<MaybeUninit<T>>,

    /// Признак инициализированности данных.
    /// [`Mutex`] должен оборачивать защищаемые данные. Тут это не так только потому что настоящее решение не содержит [`Mutex`], а манипуляции с [`OnceLock::data`] хотелось оставить в открытой части кода для упрощения задачи. // TODO: remove before flight.
    // TODO: your code here.
    initialized: AtomicU8, // TODO: remove before flight.
    // initialized: AtomicU8
}


impl<T> OnceLock<T> {
    /// Создаёт неинициализированный [`OnceLock`].
    pub const fn new() -> Self {
        Self {
            data: UnsafeCell::new(MaybeUninit::zeroed()),
            //
            initialized: AtomicU8::new(0),
        }
    }


    /// Возвращает ссылку на сохранённые данные или [`None`],
    /// если запись ещё не осуществлялась.
    pub fn get(&self) -> Option<&T> {
        if self.is_initialized() {
            Some(unsafe { (*self.data.get()).assume_init_ref() })
        } else {
            None
        }
    }


    /// Возвращает ссылку на сохранённые данные или [`None`],
    /// если запись ещё не осуществлялась.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.is_initialized() {
            Some(unsafe { (*self.data.get()).assume_init_mut() })
        } else {
            None
        }
    }


    /// Записывает данные и возвращает [`Ok`], если это первая попытка записи.
    ///
    /// Возвращает [`Error::InvalidArgument`] и не трогает данные,
    /// если попытка записи не первая.
    /// В этом случае [`OnceLock::set()`] может вернуться до того,
    /// как данные будут инициализированы.
    pub fn set(&self, value: T) -> Result<()> {
        // TODO: your code here.
        let initialize = self.initialized.compare_exchange(
            0,
            1,
            Ordering::Acquire,
            Ordering::Relaxed,
        );

        if initialize.is_ok() {
            let data = unsafe { &mut *self.data.get() };
            data.write(value);

            // // TODO: your code here.
            // *lock = true; // TODO: remove before flight.
            self.initialized.store(2, Ordering::Release);

            Ok(())
        } else {
            Err(InvalidArgument)
        }
    }


    /// Возвращает `true`, если значение уже инициализировано.
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire) == 2
    }


    // TODO: your code here.
}


impl<T> Default for OnceLock<T> {
    fn default() -> Self {
        Self::new()
    }
}


impl<T> Drop for OnceLock<T> {
    fn drop(&mut self) {
        if self.is_initialized() {
            unsafe {
                self.data.get_mut().assume_init_drop();
            }
        }
    }
}


/// См. [The Rustonomicon, "Send and Sync"](https://doc.rust-lang.org/nomicon/send-and-sync.html).
unsafe impl<T: Send> Send for OnceLock<T> {
}


/// См. [The Rustonomicon, "Send and Sync"](https://doc.rust-lang.org/nomicon/send-and-sync.html).
unsafe impl<T: Send> Sync for OnceLock<T> {
}
