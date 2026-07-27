/// Поведение блокировок при панике.
pub mod panic;

/// Примитив синхронизации [`SequenceLock`].
pub mod sequence_lock;

/// Примитив синхронизации [`Spinlock`].
pub mod spinlock;

pub use panic::{start_panicing, PanicStrategy};
pub use sequence_lock::SequenceLock;
pub use spinlock::Spinlock;
