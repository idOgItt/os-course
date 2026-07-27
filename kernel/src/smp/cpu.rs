use alloc::vec::Vec;
use core::{arch::asm, mem};
use ku::memory::mmu::PageTableFlags;

use memoffset::offset_of;
use x86_64::{
    instructions::tables,
    registers::model_specific::GsBase,
    structures::tss::TaskStateSegment,
    PrivilegeLevel,
    VirtAddr,
};

use crate::{
    error::{Error::NoProcess, Result},
    log::{debug, error},
    memory::{
        Gdt,
        Stack,
        Virt,
        BASE_ADDRESS_SPACE,
        DOUBLE_FAULT_IST_INDEX,
        EXCEPTION_STACKS,
        GDT,
        KERNEL_RW,
        PAGE_FAULT_IST_INDEX,
    },
    process::{ModeContext, Pid},
};

use super::{CpuId, LocalApic};

// Used in docs.
#[allow(unused)]
use crate::process::Registers;


/// Инициализирует вектор структур [`Cpu`] размера `cpu_count` и
/// регистры [`GS`](https://wiki.osdev.org/CPU_Registers_x86-64#FS.base.2C_GS.base) и
/// [`TR`](https://wiki.osdev.org/CPU_Registers_x86-64#TR)
/// для `current_cpu` (Bootstrap Processor).
pub(super) fn init(cpu_count: usize, current_cpu: CpuId) -> Result<Vec<Cpu>> {
    let mut cpus = init_cpu_vec(cpu_count)?;

    let cpu = &mut cpus[usize::from(current_cpu)];
    cpu.set_gs();
    cpu.set_tss();

    Ok(cpus)
}


/// Выделяет память для структур [`Cpu`] и для содержащихся в них стеках.
fn init_cpu_vec(cpu_count: usize) -> Result<Vec<Cpu>> {
    let stack_flags = PageTableFlags::WRITABLE | PageTableFlags::PRESENT;

    let stacks = Stack::new_slice(
        &mut BASE_ADDRESS_SPACE.lock(),
        stack_flags,
        cpu_count * Cpu::STACKS_PER_CPU,
    )?;

    let mut cpus = Vec::with_capacity(cpu_count);

    stacks
        .chunks(Cpu::STACKS_PER_CPU)
        .zip(0..cpu_count.try_into()?)
        .for_each(|(stack, id)| {
            cpus.push(Cpu::new(id, stack));
        });

    Ok(cpus)
}


/// Смещение внутри [`Cpu`], по которому нужно сохранять `rsp` ядра,
/// чтобы процессор переключал стек на него при возникновении прерываний в ненулевом
/// кольце защиты.
/// Ядро сохраняет там свой `rsp`, когда переключается в режим пользователя
/// и восстанавливает свой `rsp` оттуда, когда возвращается из режима пользователя
/// или получает он него системный вызов.
pub(crate) const KERNEL_RSP_OFFSET_IN_CPU: usize = offset_of!(Cpu, tss) +
    offset_of!(TaskStateSegment, privilege_stack_table) +
    PrivilegeLevel::Ring0 as usize * mem::size_of::<VirtAddr>();


/// CPU--local storage.
/// Aligned on the cache line size to avoid false sharing.
/// [Why align on 128 bytes instead of 64?](https://docs.rs/crossbeam/latest/crossbeam/utils/struct.CachePadded.html#size-and-alignment)
#[repr(align(128))]
pub(crate) struct Cpu {
    /// Исполняющийся на данном CPU в текущий момент пользовательский процесс.
    ///
    /// Пользовательские процессы выполняются независимо на разных ядрах,
    /// следовательно у каждого ядра текущий процесс должен быть своим.
    current_process: Option<Pid>,

    /// Идентификатор данного CPU, копия идентификатора его Local APIC --- [`LocalApic::id()`].
    id: CpuId,

    /// Стек ядра данного CPU.
    ///
    /// Процессоры обрабатывают системные вызовы и прерывания независимо,
    /// так что у каждого должен быть свой стек для этого.
    kernel_stack: &'static Stack,

    /// Дополнительный стек, который используется во время обработки
    /// [Page Fault](https://en.wikipedia.org/wiki/Page_fault).
    ///
    /// Он позволяет напечатать диагностику возникшего исключения даже в случае,
    /// когда оно вызвано исчерпанием основного стека ядра.
    page_fault_stack: &'static Stack,

    /// Адрес структуры [`Cpu`] для данного CPU.
    this: Virt,

    /// [Task State Segment](https://en.wikipedia.org/wiki/Task_state_segment) (TSS) данного CPU.
    ///
    /// В TSS описано где находится стек ядра для текущего процессора.
    /// Стеки у разных CPU разные, поэтому и TSS тоже должны быть разные.
    tss: TaskStateSegment,

    /// Временное хранилище для контекста принудительно вытесненного с этого CPU процесса.
    user_context: Option<ModeContext>,
}


impl Cpu {
    /// Заполняет структуру [`Cpu`] для процессора номер `id` с двумя заданными
    /// стеками `stacks` для обычного кода ядра и для обработчика
    /// [Page Fault](https://en.wikipedia.org/wiki/Page_fault).
    pub fn new(id: CpuId, stacks: &'static [Stack]) -> Self {
        assert!(stacks.len() == Self::STACKS_PER_CPU);

        let mut result = Self {
            id,
            current_process: None,
            kernel_stack: &stacks[0],
            page_fault_stack: &stacks[1],
            this: Virt::default(),
            tss: TaskStateSegment::new(),
            user_context: None,
        };

        // A double fault means there is an error in the kernel.
        // So a common double fault stack for all CPUs is sufficient as long as
        // `double_fault()` handler in `interrupts.rs` locks
        // and never unlocks a common mutex and does not return.
        result.tss.interrupt_stack_table[usize::from(DOUBLE_FAULT_IST_INDEX)] =
            EXCEPTION_STACKS.lock().double_fault_rsp().into();
        result.tss.interrupt_stack_table[usize::from(PAGE_FAULT_IST_INDEX)] =
            result.page_fault_stack.pointer().into();

        result
    }


    /// Возвращает исполняющийся на данном CPU в текущий момент пользовательский процесс.
    ///
    /// Пользовательские процессы выполняются независимо на разных ядрах,
    /// следовательно у каждого ядра текущий процесс должен быть своим.
    pub(crate) fn current_process() -> Result<Pid> {
        let cpu = unsafe { Self::get() };
        cpu.current_process.ok_or(NoProcess)
    }


    /// Устанавливает исполняющийся на данном CPU в текущий момент пользовательский процесс.
    ///
    /// Пользовательские процессы выполняются независимо на разных ядрах,
    /// следовательно у каждого ядра текущий процесс должен быть своим.
    pub(crate) fn set_current_process(process: Option<Pid>) {
        let cpu = unsafe { Self::get() };
        cpu.current_process = process;
    }


    /// Возвращает контекст ядра для данного [`Cpu`], то есть с его стеком.
    /// В качестве `rip` использует функцию [`Registers::switch_from()`],
    /// то есть при переходе в этот контекст будет запущена именно она.
    pub(crate) fn kernel_context() -> ModeContext {
        let cpu = unsafe { Self::get() };
        ModeContext::kernel_context(
            cpu.tss.privilege_stack_table[PrivilegeLevel::Ring0 as usize].into(),
        )
    }


    /// Записывает в [`Cpu`] контекст принудительно вытесненного с этого CPU процесса.
    pub(crate) fn set_user_context(user_context: ModeContext) {
        let cpu = unsafe { Self::get() };
        cpu.user_context = Some(user_context);
    }


    /// Вытаскивает из [`Cpu`] контекст принудительно вытесненного с этого CPU процесса.
    /// После выполнения этой функции в соответствующем поле [`Cpu`] будет храниться [`None`].
    pub(crate) fn take_user_context() -> Option<ModeContext> {
        let cpu = unsafe { Self::get() };
        cpu.user_context.take()
    }


    /// Идентификатор данного CPU, копия идентификатора его Local APIC --- [`LocalApic::id()`].
    pub(super) fn id(&self) -> CpuId {
        self.id
    }


    /// Стек ядра данного CPU.
    ///
    /// Процессоры обрабатывают системные вызовы и прерывания независимо,
    /// так что у каждого должен быть свой стек для этого.
    pub(super) fn kernel_stack(&self) -> &'static Stack {
        self.kernel_stack
    }


    /// Инициализация регистра
    /// [`GS`](https://wiki.osdev.org/CPU_Registers_x86-64#FS.base.2C_GS.base)
    /// текущего CPU.
    pub(super) fn set_gs(&mut self) {
        let tls = Virt::from_ref(self);
        self.this = tls;
        GsBase::write(tls.into());
    }


    /// Инициализация [Task State Segment](https://en.wikipedia.org/wiki/Task_state_segment)
    /// и регистра [`TR`](https://wiki.osdev.org/CPU_Registers_x86-64#TR) текущего CPU.
    pub(super) fn set_tss(&self) {
        assert!(self.id() == LocalApic::id());

        GDT.lock().set_tss(self.id(), &self.tss);
        let tss = Gdt::tss(self.id());

        unsafe {
            tables::load_tss(tss);
        }
    }


    /// Возвращает ссылку на структуру [`Cpu`] текущего процессора.
    unsafe fn get() -> &'static mut Cpu {
        let this: usize;

        unsafe {
            asm!(
            "mov {this}, gs:{this_offset}",
            this_offset = const offset_of!(Cpu, this),
            this = out(reg) this,
            options(nostack),
            );
        }

        let cpu = Virt::from_ptr(this as *mut Cpu);
        unsafe { cpu.try_into_mut().unwrap() }
    }


    /// Каждому CPU нужно два стека:
    ///   - для обычного кода ядра и
    ///   - для обработчика [Page Fault](https://en.wikipedia.org/wiki/Page_fault).
    const STACKS_PER_CPU: usize = 2;
}


impl Drop for Cpu {
    fn drop(&mut self) {
        panic!("a Cpu has been dropped");
    }
}


#[doc(hidden)]
pub mod test_scaffolding {
    use super::Cpu;


    pub fn cpu_id() -> u8 {
        let cpu = unsafe { Cpu::get() };
        cpu.id()
    }
}