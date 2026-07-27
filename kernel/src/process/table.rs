use alloc::vec::Vec;
use core::fmt;

use lazy_static::lazy_static;

use ku::sync::spinlock::{Spinlock, SpinlockGuard};

use crate::{
    error::{
        Error::{InvalidArgument, NoProcess, NoProcessSlot},
        Result,
    },
    log::info,
    time,
};

use super::{Pid, Process};

// Used in docs.
#[allow(unused)]
use crate::error::Error;


/// Слот таблицы процессов.
#[allow(clippy::large_enum_variant)]
enum Slot {
    /// Слот свободен.
    Free {
        /// Хранит эпоху последнего процесса, занимавшего этот слот.
        pid: Pid,

        /// Провязывает свободные слоты в интрузивный список.
        next: Option<Pid>,
    },

    /// Слот занят.
    Occupied {
        /// Процесс, находящийся в этом слоте таблицы процессов.
        process: Spinlock<Process>,
    },
}


impl fmt::Display for Slot {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Slot::Free { pid, next } => {
                write!(formatter, "Free {{ pid: {}, next: {:?} }}", pid, next)
            },
            Slot::Occupied { process } => write!(formatter, "Process {}", *process.lock()),
        }
    }
}


/// Таблица процессов.
#[derive(Default)]
pub struct Table {
    /// Голова списка свободных слотов таблицы.
    free: Option<Pid>,

    /// Количество процессов в таблице.
    process_count: usize,

    /// Слоты таблицы процессов.
    table: Vec<Slot>,
}


impl Table {
    /// Он создаёт таблицу процессов [`Table`] размера `len` элементов, заполняя её
    /// пустыми слотами [`Slot::Free`] с соответствующими индексам слотов полями [`Pid::Id::slot`].
    /// Эти пустые слоты провязывает в односвязный список с головой в поле [`Table::free`].
    pub(super) fn new(len: usize) -> Self {
        // TODO: your code here.
        unimplemented!();
    }


    /// Выделяет процессу `process` свободный слот таблицы и возвращает соответствующий [`Pid`].
    /// Если свободного слота нет, возвращает ошибку [`Error::NoProcessSlot`].
    #[allow(unused_mut)] // TODO: remove before flight.
    pub(super) fn allocate(mut process: Process) -> Result<Pid> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Удаляет процесс с заданным `pid`.
    /// При этом:
    ///   - Инкрементирует эпоху в освободившемся слоте.
    ///   - Вставляет слот в голову списка свободных слотов [`Table::free`].
    #[allow(unused_mut)] // TODO: remove before flight.
    pub fn free(mut pid: Pid) -> Result<()> {
        // TODO: your code here.

        Ok(())
    }


    /// Возвращает заблокированный спинлок [`SpinlockGuard`] со структурой [`Process`]
    /// соответствующей идентификатору `pid`.
    /// Если процесса по указанному `pid` нет или тот же слот занят уже другим процессом,
    /// возвращает ошибку [`Error::NoProcess`].
    pub fn get(pid: Pid) -> Result<SpinlockGuard<'static, Process>> {
        // TODO: your code here.
        test_scaffolding::process() // TODO: remove before flight.
    }
}


impl Drop for Table {
    fn drop(&mut self) {
        assert!(
            self.table.is_empty(),
            "should not drop non-empty process table"
        );
    }
}


/// Обещает Rust, что слоты таблицы процессов [`Table`] имеют время жизни `'static`,
/// так как Rust не может проверить это самостоятельно.
///
/// Так как размер таблицы процессов [`Table`] после инициализации мы никода не меняем,
/// и в частности не уменьшаем, время жизни каждого её слота --- практически `'static`.
unsafe fn forge_static_lifetime<T>(x: &T) -> &'static T {
    unsafe { &*(x as *const T) as &'static T }
}


lazy_static! {
    /// Таблица процессов.
    pub(super) static ref TABLE: Spinlock<Table> = Spinlock::new(Table::default());
}


#[doc(hidden)]
pub mod test_scaffolding {
    use lazy_static::lazy_static;

    use ku::sync::spinlock::{Spinlock, SpinlockGuard};

    use crate::error::{Error::NoProcess, Result};

    use super::{forge_static_lifetime, Pid, Process, TABLE};


    pub(super) fn process() -> Result<SpinlockGuard<'static, Process>> {
        assert!(TABLE.lock().table.is_empty());

        DUMMY_TABLE
            .lock()
            .as_ref()
            .map(|process| unsafe { forge_static_lifetime(process).lock() })
            .ok_or(NoProcess)
    }


    pub fn set_process(mut process: Process) {
        assert!(TABLE.lock().table.is_empty());

        process.set_pid(Pid::new(0));
        *DUMMY_TABLE.lock() = Some(Spinlock::new(process));
    }


    lazy_static! {
        pub(super) static ref DUMMY_TABLE: Spinlock<Option<Spinlock<Process>>> =
            Spinlock::new(None);
    }
}
