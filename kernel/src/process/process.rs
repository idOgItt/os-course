use core::fmt;

use ku::{
    process::{Info, MiniContext, State, TrapInfo},
    sync::spinlock::{Spinlock, SpinlockGuard},
    ProcessInfo,
    ReadBuffer,
};

use crate::{
    allocator::MemoryAllocator,
    error::{Error::InvalidArgument, Result},
    log::{self, debug, info, trace, warn},
    memory::{mmu::PageTableFlags, AddressSpace, Block, Virt, USER_RW},
    smp::Cpu,
    trap::{self, Trap, TRAP_STATS},
};

use super::{registers::Registers, Pid, Table};


/// Описывает пользовательский процесс.
#[derive(Debug)]
pub struct Process {
    /// Виртуальное адресное пространство процесса.
    address_space: Spinlock<AddressSpace>,

    /// Блок памяти, через который ядро предоставляет процессу информацию о нём.
    /// В этом блоке находится структура типа [`ProcessInfo`].
    info: Block<Virt>,

    /// Буфер, в который код пользователя записывает свои логи.
    log: ReadBuffer,

    /// Идентификатор процесса--родителя, который создал данный процесс.
    parent: Option<Pid>,

    /// Идентификатор процесса.
    pid: Pid,

    /// Состояние регистров процесса.
    registers: Registers,

    /// Состояние процесса.
    state: State,

    /// Контекст пользователя, в который передаются исключения и прерывания,
    /// относящиеся к данному процессу.
    /// Например, Page Fault при некорректном доступе к памяти в коде пользователя.
    trap_context: TrapContext,
}


impl Process {
    /// Создаёт новый процесс.
    pub(super) fn new(
        address_space: AddressSpace,
        context: MiniContext,
        info: &'static mut ProcessInfo,
        log: ReadBuffer,
    ) -> Self {
        let info = Block::from_mut(info);

        Self {
            address_space: Spinlock::new(address_space),
            info,
            log,
            parent: None,
            pid: Pid::Current,
            registers: Registers::new(context, info.start_address().into_usize()),
            state: State::Runnable,
            trap_context: TrapContext::default(),
        }
    }


    /// Дублирует существующий процесс.
    pub(super) fn duplicate(&mut self, rax: usize, rdi: usize) -> Result<Self> {
        let stack = if let Ok(info) = unsafe { self.info() } {
            info.stack()
        } else {
            Block::default()
        };

        let mut address_space = self.address_space.lock().duplicate()?;

        address_space.switch_to();
        let (read_buffer, write_buffer) = super::map_log(&mut address_space)?;
        let info = super::map_process_info(&mut address_space, stack, write_buffer)?;
        let info = Block::from_mut(info);
        self.address_space.lock().switch_to();

        Ok(Self {
            address_space: Spinlock::new(address_space),
            info,
            log: read_buffer,
            parent: Some(self.pid),
            pid: Pid::Current,
            registers: self.registers.duplicate(rax, rdi, info.start_address().into_usize()),
            state: State::Exofork,
            trap_context: TrapContext::default(),
        })
    }


    /// Возвращает виртуальное адресное пространство процесса.
    pub fn address_space(&mut self) -> &mut AddressSpace {
        self.address_space.get_mut()
    }


    /// Захватывает и возвращает блокировку на виртуальное адресное пространство процесса.
    /// В отличие от [`Process::address_space()`] не требует эксклюзивную ссылку на [`Process`].
    /// Это позволяет одновременно держать ссылки и на адресное пространство процесса и
    /// на его аллокатор памяти [`MemoryAllocator`].
    pub(super) fn lock_address_space(&self) -> SpinlockGuard<'_, AddressSpace> {
        self.address_space.lock()
    }


    /// Возвращает аллокатор памяти общего назначения внутри адресного пространства процесса.
    pub(super) fn allocator(&self, flags: PageTableFlags) -> MemoryAllocator {
        MemoryAllocator::new(&self.address_space, flags)
    }


    /// Устанавливает минимальный контекст процесса.
    pub(super) fn set_context(&mut self, context: MiniContext) {
        self.registers.set_mini_context(context);
    }


    /// Возвращает идентификатор процесса--родителя, который создал данный процесс.
    pub fn parent(&self) -> Option<Pid> {
        self.parent
    }


    /// Возвращает идентификатор процесса.
    pub fn pid(&self) -> Pid {
        assert!(
            self.pid != Pid::Current,
            "the process has not been assigned a pid yet"
        );
        self.pid
    }


    /// Устанавливает идентификатор процесса.
    pub(super) fn set_pid(&mut self, pid: Pid) {
        if let Pid::Id { .. } = pid {
            self.pid = pid;
            self.address_space.get_mut().set_pid(pid);

            test_scaffolding::pid_callback(self);
        } else {
            panic!("assignment of a wrong pid to a process");
        }
    }


    /// Возвращает состояние процесса.
    pub(super) fn state(&mut self) -> State {
        self.state
    }


    /// Устанавливает состояние процесса.
    pub(super) fn set_state(&mut self, state: State) {
        self.state = state
    }


    /// Возвращает контекст пользователя, в который передаются исключения и прерывания,
    /// относящиеся к данному процессу.
    /// Например, Page Fault при некорректном доступе к памяти в коде пользователя.
    pub(super) fn trap_context(&self) -> TrapContext {
        self.trap_context
    }


    /// Устанавливает контекст пользователя, в который передаются исключения и прерывания,
    /// относящиеся к данному процессу.
    /// Например, Page Fault при некорректном доступе к памяти в коде пользователя.
    pub(super) fn set_trap_context(&mut self, trap_context: TrapContext) {
        self.trap_context = trap_context;
    }


    /// Переходит в режим пользователя.
    /// Возвращает `true` если возврат из режима пользователя
    /// был совершён принудительно (по прерыванию таймера `trap::apic_timer()`).
    pub fn enter_user_mode(mut process: SpinlockGuard<Process>) -> bool {
        let pid = process.pid;

        process.address_space.get_mut().switch_to();

        if let Ok(info) = unsafe { process.info() } {
            info.set_pid(pid);
        }

        Cpu::set_current_process(Some(pid));

        let registers = &process.registers as *const Registers;

        process.state = State::Running;

        debug!(%pid, registers = %process.registers, "entering the user mode");

        drop(process);

        unsafe {
            Registers::switch_to(registers);
        }

        debug!(%pid, "leaving the user mode");

        Cpu::set_current_process(None);

        // TODO: your code here.

        false
    }


    /// Выполняет переключение текущего контекста исполнения `context`
    /// на контекст ядра в случае, если текущий контекст исполняется в режиме пользователя.
    /// Текущий контекст пользователя сохраняет в структуре `Cpu` текущего процессора.
    ///
    /// Этот метод вызывается из функции обработки прерывания от таймера `trap::apic_timer()`.
    #[allow(unused_mut)] // TODO: remove before flight.
    #[inline(always)]
    pub(crate) fn preempt(context: &mut trap::TrapContext) {
        // TODO: your code here.
    }


    /// Вытесняет текущий исполняющийся процесс с процессора по его собственному запросу.
    ///
    /// Не сохраняет контекст процесса пользователя,
    /// он должен сохранить его сам в процедуре системного вызова `syscall::sched_yield()`.
    /// Возвращается в контекст ядра, из которого этот процесс был запущен.
    /// Текущий контекст ядра уничтожается.
    pub(crate) extern "C" fn sched_yield() -> ! {
        unsafe {
            assert!(Cpu::current_process().is_ok());

            Registers::sched_yield();
        }
    }


    /// Подготавливает контекст `context` к вызову
    /// пользовательского обработчика исключения или прерывания номер `trap`, если он установлен.
    ///
    /// Возвращает `true`, если пользовательский обработчик установлен и
    /// инфромация об исключении или прерывании успешно записана в его стек.
    #[allow(unused_mut)] // TODO: remove before flight.
    pub(crate) fn trap(&mut self, context: &mut trap::TrapContext, trap: Trap, info: Info) -> bool {
        self.flush_log();

        let number = usize::from(trap);

        // TODO: your code here.
        false // TODO: remove before flight.
    }


    /// Сбрасывает буферизованные записи из пользовательского пространства в лог.
    pub(super) fn flush_log(&mut self) {
        let pid = self.pid;

        if let Ok(log) = self.log() {
            log::user_events(pid, log);
            trace!(read_stats = ?*log.read_stats());
        } else {
            warn!(%pid, "log is not mapped properly");
        }
    }


    /// Возвращает ссылку на структуру [`ProcessInfo`],
    /// через которую ядро предоставляет процессу информацию о нём.
    unsafe fn info(&mut self) -> Result<&mut ProcessInfo> {
        let info = self
            .address_space
            .get_mut()
            .check_permission_mut::<ProcessInfo>(self.info, USER_RW)?;

        if info.len() == 1 {
            Ok(&mut info[0])
        } else {
            Err(InvalidArgument)
        }
    }


    /// Возвращает буфер, в который код пользователя записывает свои логи.
    fn log(&mut self) -> Result<&mut ReadBuffer> {
        self.address_space
            .get_mut()
            .check_permission_mut::<u8>(self.log.block().into(), USER_RW)?;
        Ok(&mut self.log)
    }
}


impl fmt::Display for Process {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{{ pid: {}, address_space: {}, {} }}",
            self.pid,
            self.address_space.lock(),
            self.registers.mini_context(),
        )
    }
}


/// Контекст пользователя, в который передаются исключения и прерывания,
/// относящиеся к данному процессу.
/// Например, Page Fault при некорректном доступе к памяти в коде пользователя.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct TrapContext {
    /// Контекст пользовательского обработчика исключений и прерываний.
    mini_context: MiniContext,

    /// Блок памяти, отведённый под стек для
    /// пользовательского обработчика исключений и прерываний.
    stack: Block<Virt>,
}


impl TrapContext {
    /// Создаёт контекст пользовательского обработчика исключений и прерываний.
    pub(super) fn new(rip: Virt, stack: Block<Virt>) -> Result<TrapContext> {
        Ok(Self {
            mini_context: MiniContext::new(rip, stack.end_address()?),
            stack,
        })
    }


    /// Возвращает `true` если указатель стека `rsp` указывает в стек
    /// пользовательского обработчика исключений и прерываний.
    fn contains(&self, rsp: Virt) -> bool {
        self.stack.contains_address(rsp)
    }


    /// Возвращает `true` если пользователь установил свой обработчик исключений.
    fn is_valid(&self) -> bool {
        self.stack != Block::default()
    }


    /// Возвращает контекст пользовательского обработчика исключений и прерываний.
    fn mini_context(&self) -> MiniContext {
        self.mini_context
    }
}


impl fmt::Display for TrapContext {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{{ {}, stack: {} }}",
            self.mini_context, self.stack,
        )
    }
}


#[doc(hidden)]
pub(super) mod test_scaffolding {
    use core::{
        mem,
        ptr,
        sync::atomic::{AtomicPtr, Ordering},
    };

    use static_assertions::const_assert;

    use super::{super::registers::test_scaffolding, Pid, Process};


    pub fn disable_interrupts(process: &mut Process) {
        test_scaffolding::disable_interrupts(&mut process.registers);
    }


    pub fn registers(process: &Process) -> [usize; 15] {
        test_scaffolding::registers(&process.registers)
    }


    pub fn set_pid(process: &mut Process, pid: Pid) {
        process.set_pid(pid);
    }


    pub fn set_pid_callback(pid_callback: fn(&Process)) {
        PID_CALLBACK.store(pid_callback as *mut _, Ordering::Relaxed);
    }


    pub(super) fn pid_callback(process: &Process) {
        let pid_callback = PID_CALLBACK.load(Ordering::Relaxed);
        if !pid_callback.is_null() {
            unsafe {
                const_assert!(mem::size_of::<*const ()>() == mem::size_of::<fn(&Process)>());
                let pid_callback = mem::transmute::<*const (), fn(&Process)>(pid_callback);
                (pid_callback)(process);
            }
        }
    }


    static PID_CALLBACK: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());
}
