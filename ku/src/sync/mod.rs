/// Примитив синхронизации [`OnceLock`] для данных,
/// которые один раз записываются, а потом только читаются.
pub mod once_lock;

/// Поведение блокировок при панике.
pub mod panic;

/// Примитив синхронизации [`SequenceLock`].
pub mod sequence_lock;

/// Примитив синхронизации [`Spinlock`].
pub mod spinlock;

pub use once_lock::OnceLock;
pub use panic::{start_panicing, PanicStrategy};
pub use sequence_lock::SequenceLock;
pub use spinlock::Spinlock;
