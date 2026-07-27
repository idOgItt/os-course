use core::{
    cell::UnsafeCell,
    hint,
    sync::atomic::{AtomicU64, Ordering},
};

use atomic_memcpy;

// Used in docs.
#[allow(unused)]
use super::spinlock::Spinlock;


/// Реализует блокировку [sequence lock](https://en.wikipedia.org/wiki/Seqlock)
/// для согласованного доступа к разделяемым данным.
///
/// Она позволяет не захватываь блокировку в читателе,
/// поэтому писатели никогда не ждут читателей.
///
/// См. также:
///   - [Writing a seqlock in Rust.](https://pitdicker.github.io/Writing-a-seqlock-in-Rust/)
///   - [Can Seqlocks Get Along With Programming Language Memory Models?](https://www.hpl.hp.com/techreports/2012/HPL-2012-68.pdf)
///   - [Crate seqlock.](https://docs.rs/seqlock/0.1.2/seqlock/)
#[derive(Debug)]
pub struct SequenceLock<T: Copy> {
    /// Защищаемые данные.
    data: UnsafeCell<T>,

    /// Возрастающая последовательность чисел, которая позволяет понять:
    ///   - Взята ли блокировка на запись.
    ///   - Консистентно ли прочитаны данные.
    sequence: AtomicU64,
}


impl<T: Copy> SequenceLock<T> {
    /// Создаёт новый [`SequenceLock`] для защиты `data`.
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            sequence: AtomicU64::new(0),
        }
    }


    /// Захватывает блокировку на запись.
    /// При этом ожидает в активном цикле освобождения блокировки, если она уже захвачена.
    ///
    /// Возвращает [`SequenceLockGuard`], который:
    ///   - Позволяет читать и писать в защищаемые [`SequenceLock`] данные
    ///     методами [`SequenceLockGuard::get()`] и [`SequenceLockGuard::set()`] соответственно.
    ///   - Автоматически освобождает блокировку в реализации типажа [`Drop`].
    pub fn write_lock(&self) -> SequenceLockGuard<'_, T> {
        loop {
            let seq = self.sequence.load(Ordering::Acquire);

            if !Self::is_locked(seq) {
                if self.sequence.compare_exchange(
                    seq,
                    seq + 1,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ).is_ok() {
                    break;
                }
            }
            core::hint::spin_loop();
        }
        // TD
        SequenceLockGuard { sequence_lock : self }
    }


    /// Помечает [`SequenceLock`] как записываемый в текущий момент,
    /// если вызывающая сторона может гарантировать,
    /// что [`SequenceLock`] не захвачен на запись.
    ///
    /// Полезна, если писатель только один, или синхронизация писателей достигается не
    /// с помощью [`SequenceLock`], а каким-то другим способом.
    /// Например, [`Spinlock`] использует этот метод в момент удержания собственной блокировки.
    ///
    /// Возвращает [`SequenceLockGuard`], который:
    ///   - Позволяет читать и писать в защищаемые [`SequenceLock`] данные
    ///     методами [`SequenceLockGuard::get()`] и [`SequenceLockGuard::set()`] соответственно.
    ///   - Автоматически освобождает блокировку в реализации типажа [`Drop`].
    ///
    /// # Safety
    ///
    /// Вызывающая сторона должна быть уже синхронизированна с другими писателями.
    /// То есть, обеспечить эксклюзивность записи.
    ///
    /// # Panics
    ///
    /// Паникует, если обнаруживается, что вызывающая сторона не обладает эксклюзивностью записи.
    pub unsafe fn write(&self) -> SequenceLockGuard<'_, T> {
        let seq = self.sequence.load(Ordering::Acquire);
        if Self::is_locked(seq) {
            panic!("Sequence lock already acquired for writing!");
        }

        self.sequence.store(seq + 1, Ordering::Release);
        SequenceLockGuard {sequence_lock : self}
    }


    /// Читает защищаемые [`SequenceLock`] данные.
    /// При этом в активном цикле ожидает освобождения блокировки на запись, если она захвачена.
    pub fn read(&self) -> T {
        loop {
            let seq_start = self.sequence.load(Ordering::Acquire);

            if Self::is_locked(seq_start) {
                continue;
            }

            let data = unsafe { atomic_memcpy::atomic_load(self.data.get(), Ordering::Acquire)};

            let seq_end = self.sequence.load(Ordering::Relaxed);

            if seq_start == seq_end && !Self::is_locked(seq_start) {
                return unsafe { data.assume_init() };
            }
        }
    }


    /// Пытается прочитать защищаемые [`SequenceLock`] данные.
    /// Возвращает [`None`], если конкурентно захвачена блокировка на запись.
    fn try_read(&self) -> Option<T> {
        let seq_start = self.sequence.load(Ordering::Acquire);

        if Self::is_locked(seq_start) {
            return None;
        }

        let data = unsafe { atomic_memcpy::atomic_load(self.data.get(), Ordering::Acquire)};
        let seq_end = self.sequence.load(Ordering::Acquire);

        if seq_start == seq_end && !Self::is_locked(seq_start) {
            return Some(unsafe { data.assume_init() });
        }

        None
    }


    /// Позволяет читать и писать в защищаемые [`SequenceLock`] данные без блокирования в случае,
    /// если вызывающий код эксклюзивно владеет [`SequenceLock`] --- `&mut self`.
    /// То есть, когда конкурентного доступа к [`SequenceLock`] быть не может.
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }


    /// Возвращает `true`, если значение `sequence` означает,
    /// что захвачена блокировка на запись.
    fn is_locked(sequence: u64) -> bool {
        sequence % 2 != 0
    }
}


/// См. [The Rustonomicon, "Send and Sync"](https://doc.rust-lang.org/nomicon/send-and-sync.html).
unsafe impl<T: Copy + Send> Send for SequenceLock<T> {
}


/// См. [The Rustonomicon, "Send and Sync"](https://doc.rust-lang.org/nomicon/send-and-sync.html).
unsafe impl<T: Copy + Send> Sync for SequenceLock<T> {
}


/// Захваченный на запись [`SequenceLock`].
///
/// - Позволяет читать и писать в защищаемые [`SequenceLock`] данные
///   методами [`SequenceLockGuard::get()`] и [`SequenceLockGuard::set()`] соответственно.
/// - Автоматически освобождает блокировку в реализации типажа [`Drop`].
pub struct SequenceLockGuard<'a, T: Copy> {
    /// Захваченный на запись [`SequenceLock`].
    sequence_lock: &'a SequenceLock<T>,
}


impl<T: Copy> SequenceLockGuard<'_, T> {
    /// Читает защищаемые [`SequenceLock`] данные.
    pub fn get(&self) -> T {
        let data = unsafe {
            atomic_memcpy::atomic_load(self.sequence_lock.data.get(), Ordering::Relaxed)
        };
        unsafe {data.assume_init()}
    }


    /// Записывает защищаемые [`SequenceLock`] данные.
    pub fn set(&mut self, value: T) {
        unsafe {
            atomic_memcpy::atomic_store(self.sequence_lock.data.get(), value, Ordering::Release);
        }
    }
}


impl<T: Copy> Drop for SequenceLockGuard<'_, T> {
    fn drop(&mut self) {
        self.sequence_lock.sequence.fetch_add(1, Ordering::Release);
    }
}
