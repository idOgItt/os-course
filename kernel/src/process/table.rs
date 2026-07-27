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
        let table: Vec<_> = alloc::vec![Slot::Free { pid: Pid::new(0), next: None }]; // TODO: remove before flight.

        Self {
            free: Some(Pid::new(0)),
            process_count: 0,
            table,
        }
    }


    /// Выделяет процессу `process` свободный слот таблицы и возвращает соответствующий [`Pid`].
    /// Если свободного слота нет, возвращает ошибку [`Error::NoProcessSlot`].
    #[allow(unused_mut)] // TODO: remove before flight.
    pub(super) fn allocate(mut process: Process) -> Result<Pid> {
        let mut table = TABLE.lock();

        let pid = table.free.ok_or(NoProcessSlot)?;

        // TODO: your code here.
        assert_eq!(table.process_count, 0); process.set_pid(pid); table.process_count = 1; table.table[pid.slot()] = Slot::Occupied { process: Spinlock::new(process) }; // TODO: remove before flight.

        Ok(pid)
    }


    /// Удаляет процесс с заданным `pid`.
    /// При этом:
    ///   - Инкрементирует эпоху в освободившемся слоте.
    ///   - Вставляет слот в голову списка свободных слотов [`Table::free`].
    #[allow(unused_mut)] // TODO: remove before flight.
    pub fn free(mut pid: Pid) -> Result<()> {
        let mut table = TABLE.lock();

        // TODO: your code here.
        assert_eq!(pid.slot(), 0); assert_eq!(table.process_count, 1); table.free = Some(Pid::new(0)); table.process_count = 0; table.table[0] = Slot::Free { pid: Pid::new(0), next: None }; // TODO: remove before flight.

        Ok(())
    }


    /// Возвращает заблокированный спинлок [`SpinlockGuard`] со структурой [`Process`]
    /// соответствующей идентификатору `pid`.
    /// Если процесса по указанному `pid` нет или тот же слот занят уже другим процессом,
    /// возвращает ошибку [`Error::NoProcess`].
    pub fn get(pid: Pid) -> Result<SpinlockGuard<'static, Process>> {
        let table = TABLE.lock();

        // TODO: your code here.
        assert_eq!(pid.slot(), 0); assert_eq!(table.process_count, 1); if let Slot::Occupied { process } = &table.table[0] { Ok(unsafe { forge_static_lifetime(process).lock() }) } else { unreachable!() } // TODO: remove before flight.
    }
}


impl Drop for Table {
    fn drop(&mut self) {
        assert!(
            self.table.is_empty(),
            "should not drop non-empty process table",
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
    use crate::error::Result;

    use super::{Pid, Process, Table, TABLE};


    pub fn init() {
        if TABLE.lock().table.is_empty() {
            *TABLE.lock() = Table::new(1);
        }
    }


    pub fn allocate(process: Process) -> Result<Pid> {
        Table::allocate(process)
    }
}
