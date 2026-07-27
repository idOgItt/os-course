use core::{
    arch,
    arch::asm,
    fmt,
    hint,
    mem,
    sync::atomic::{AtomicBool, Ordering},
};

use chrono::Duration;
use memoffset::offset_of;
use static_assertions::const_assert;
use x86::msr::IA32_EFER;
use x86_64::{
    instructions::interrupts,
    registers::{
        control::{Cr0, Cr4},
        model_specific::Efer,
    },
    structures::gdt::SegmentSelector,
};

#[cfg(not(feature = "conservative-backtraces"))]
use macros::with_sentinel_frame;

use crate::{
    error::{Error::Timeout, Result},
    log::{debug, error, info},
    memory,
    memory::{
        gdt::SmallGdt,
        size,
        Block,
        Gdt,
        Page,
        Phys,
        RealModePseudoDescriptor,
        Virt,
        BASE_ADDRESS_SPACE,
        GDT,
        KERNEL_RW,
    },
    process::{syscall, Scheduler},
    time,
    trap::IDT,
};

use super::{cpu::Cpu, LocalApic};

// Used in docs.
#[allow(unused)]
use crate::error::Error;


/// Запуск Application Processor с Bootstrap Processor.
/// Аргумент `phys2virt` --- виртуальная страница, с которой начинается линейное отображение
/// всей физической памяти в виртуальную, см. [`memory::phys2virt_map()`].
/// Аргумент `cpu` задаёт структуру [`Cpu`] для запускаемого Application Processor.
///
/// MultiProcessor Specification 4.1 part B.4 "Application Processor Startup"
pub(super) fn boot_ap(phys2virt: Page, cpu: &mut Cpu) -> Result<()> {
    let cpu_id = cpu.id();
    let code_size = switch_mode_code_size();
    let code_size_is_unrealistic = code_size < 8;
    if code_size_is_unrealistic {
        debug!(cpu = cpu_id, %code_size, "AP init does not seem to be implemented, skipping it");
        return Ok(());
    }

    let timer = time::timer();
    let initialized = AtomicBool::new(false);

    let (boot_code, _saved_memory) = prepare_boot_code(phys2virt, cpu, &initialized)?;

    LocalApic::send_init(cpu_id, boot_code)?;

    let message = "boot Application Processor";

    if let Err(error) = wait_initialized(&initialized, Duration::seconds(1)) {
        error!(cpu = cpu_id, duration = %timer.elapsed(), ?error, message);
    } else {
        info!(cpu = cpu_id, duration = %timer.elapsed(), message);
    }

    Ok(())
}


/// Подготовка кода и стека начальной инициализации Application Processor.
/// Возвращает физический адрес, куда сохранён код и стек инициализации.
/// А также гард, который при своём удалении восстановит исходное содержимое этой памяти.
/// Аргумент `phys2virt` --- виртуальная страница, с которой начинается линейное отображение
/// всей физической памяти в виртуальную, см. [`memory::phys2virt_map()`].
/// Аргумент `cpu` задаёт структуру [`Cpu`] для запускаемого Application Processor.
/// Через `initialized` запускаемый Application Processor сигнализирует
/// запускающему Bootstrap Processor, что инициализация AP завершена.
fn prepare_boot_code(
    phys2virt: Page,
    cpu: &mut Cpu,
    initialized: &AtomicBool,
) -> Result<(Phys, impl Drop)> {
    let boot_code = real_mode_address(BOOT_CODE, "boot code");

    let boot_code_virt = memory::phys2virt_map(phys2virt, boot_code);
    let saved_memory = save_memory(Block::new(
        boot_code_virt,
        (boot_code_virt + BOOT_CODE_PLUS_STACK_SIZE)?,
    )?)?;

    copy_switch_mode_code(boot_code_virt)?;

    let boot_stack_phys = real_mode_address(BOOT_STACK, "boot stack");
    let boot_stack = BootStack::new(boot_stack_phys, cpu, initialized)?;

    debug!(?boot_stack);

    unsafe {
        memory::phys2virt_map(phys2virt, boot_stack_phys)
            .try_into_mut_ptr::<BootStack>()
            .expect("BOOT_STACK address is not sutable for BootStack type")
            .write_volatile(boot_stack);
        arch::x86_64::_mm_mfence();
    }

    Ok((boot_code, saved_memory))
}


/// Адрес, куда релоцируется код функции [`switch_from_real_mode_to_long_mode()`].
/// Чтобы он был доступен из реального режима работы, в котором стартует Application Processor.
const BOOT_CODE: usize = 7 * Page::SIZE;

/// Адрес структуры [`BootStack`] с параметрами для кода инициализации
/// [`switch_from_real_mode_to_long_mode()`].
const BOOT_STACK: usize = BOOT_CODE + BOOT_CODE_PLUS_STACK_SIZE - mem::size_of::<BootStack>();

/// Размер в памяти, который должны занимать код инициализации
/// [`switch_from_real_mode_to_long_mode()`] и необходимые ему параметры [`BootStack`].
const BOOT_CODE_PLUS_STACK_SIZE: usize = Page::SIZE;


/// Стек с дополнительной информацией для загрузки Application Processor.
///
/// Через этот стек в [`switch_from_real_mode_to_long_mode()`] передаётся дополнительная информация,
/// необходимая для загрузки AP.
/// Например, дескриптор описывающий GDT, который можно передать инструкции `lgdt` как есть.
/// Или адрес корневой таблицы страниц, который можно как есть записать в `CR3`.
/// Это сделано, чтобы:
///   - Не нужно было в функции [`switch_from_real_mode_to_long_mode()`]
///     на 16-битном ассемблере строить GDT, таблицы страниц и т.д.
///     То есть для максимального упрощения ассемблерной части кода.
///   - Содержимое системных регистров процессора на всех AP совпадало с аналогичными регистрами BSP.
///     То есть, чтобы можно было один раз правильно настроить системные регистры BSP,
///     и автоматически получить правильные и консистентные по системе в целом
///     настройки всех процессоров.
///
/// Поля в [`BootStack`] организованы в удобном для
/// [`switch_from_real_mode_to_long_mode()`] порядке.
/// В частности, поля с селектором кода [`BootStack::kernel_code`] и релоцированным адресом
/// [`BootStack::set_cs_rip_to_64bit`] метки `set_cs_rip_to_64bit:` лежат так, что образуют
/// [far pointer](https://en.wikipedia.org/wiki/Far_pointer) `kernel_code:set_cs_rip_to_64bit`,
/// который может быть использован инструкцией `far ret` как есть.
#[repr(C)]
struct BootStack {
    /// <https://wiki.osdev.org/CPU_Registers_x86-64#IA32_EFER>
    efer: u32,

    /// <https://wiki.osdev.org/CPU_Registers_x86-64#CR4>
    cr4: u32,

    /// <https://wiki.osdev.org/CPU_Registers_x86-64#CR3>
    cr3: u32,

    /// <https://wiki.osdev.org/CPU_Registers_x86-64#CR0>
    cr0: u32,

    /// Дескриптор, описывающий адрес и размер промежуточной
    /// [Global Descriptor Table](https://en.wikipedia.org/wiki/Global_Descriptor_Table) (GDT),
    /// которую использует ассемблерный код.
    /// Сама промежуточная GDT хранится тут же, в поле [`BootStack::small_gdt`].
    gdt_pseudo_descriptor: RealModePseudoDescriptor,

    /// Селектор сегмента данных ядра.
    kernel_data: SegmentSelector,

    /// Релоцированный адрес метки `set_cs_rip_to_64bit:`.
    ///
    /// [`BootStack::set_cs_rip_to_64bit`] и [`BootStack::kernel_code`] лежат так, что образуют
    /// [far pointer](https://en.wikipedia.org/wiki/Far_pointer)
    /// `kernel_code:set_cs_rip_to_64bit`,
    /// который может быть использован инструкцией `retf` как есть.
    set_cs_rip_to_64bit: u16,

    /// Селектор сегмента кода ядра.
    kernel_code: SegmentSelector,

    /// Выравнивание для следующего поля --- [`BootStack::cpu`].
    _padding: u32,

    /// Указатель на структуру [`Cpu`] запускаемого Application Processor.
    cpu: Virt,

    /// Указатель на атомарный булев флаг, через который запускаемый
    /// Application Processor сигнализирует запускающему Bootstrap Processor,
    /// что инициализация AP завершена.
    initialized: Virt,

    /// Стек ядра для запускаемого Application Processor.
    kernel_stack: Virt,

    /// Промежуточная
    /// [Global Descriptor Table](https://en.wikipedia.org/wiki/Global_Descriptor_Table) (GDT),
    /// которую использует ассемблерный код.
    small_gdt: SmallGdt,
}


const_assert!(offset_of!(BootStack, cpu) % mem::size_of::<Virt>() == 0);


impl BootStack {
    /// Подготавливает временный стек для функции [`switch_from_real_mode_to_long_mode()`].
    /// Аргумент `boot_stack_phys` указывает физический адрес, куда будет помещён этот стек.
    /// Аргумент `cpu` задаёт структуру [`Cpu`] для запускаемого Application Processor.
    /// Через `initialized` запускаемый Application Processor сигнализирует
    /// запускающему Bootstrap Processor, что инициализация AP завершена.
    fn new(boot_stack_phys: Phys, cpu: &mut Cpu, initialized: &AtomicBool) -> Result<Self> {
        // Declare variables for assembly output
        let (switch_mode_start, set_cs_rip_to_64bit): (usize, usize);

        // Inline assembly to obtain addresses of specific labels
        unsafe {
            asm!(
            "
            mov {switch_mode_start}, OFFSET switch_mode_start
            mov {set_cs_rip_to_64bit}, OFFSET set_cs_rip_to_64bit
            ",
            switch_mode_start = out(reg) switch_mode_start,
            set_cs_rip_to_64bit = out(reg) set_cs_rip_to_64bit,
            )
        }

        // Read various control registers and set them to variables, converting as necessary
        let efer = Efer::read().bits().try_into()?;
        let cr4 = Cr4::read_raw().try_into()?;
        let cr3 = BASE_ADDRESS_SPACE.lock().page_table_root().address().try_into()?;
        let cr0 = Cr0::read_raw().try_into()?;

        // Setup the GDT (Global Descriptor Table)
        let small_gdt = SmallGdt::new();
        let gdt_pseudo_descriptor = SmallGdt::real_mode_pseudo_descriptor(
            (boot_stack_phys + offset_of!(BootStack, small_gdt))?,
        )?;

        // Calculate the 16-bit offset for `set_cs_rip_to_64bit`
        let set_cs_rip_to_64bit_offset: u16 = size::into_u64(
            BOOT_CODE + (set_cs_rip_to_64bit - switch_mode_start),
        )
            .try_into()
            .expect("set_cs_rip_to_64bit label address should fit into 16 bit for Application Processors to be able to ret to it");

        // Retrieve kernel stack pointer for the CPU
        let kernel_stack = cpu.kernel_stack().pointer();

        // Construct and return the new instance of the structure
        Ok(Self {
            efer,
            cr4,
            cr3,
            cr0,
            gdt_pseudo_descriptor,
            kernel_data: Gdt::kernel_data(),
            set_cs_rip_to_64bit: set_cs_rip_to_64bit_offset,
            kernel_code: Gdt::kernel_code(),
            _padding: 0,
            cpu: Virt::from_mut(cpu),
            initialized: Virt::from_ref(initialized),
            kernel_stack,
            small_gdt,
        })
    }
}


impl fmt::Debug for BootStack {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{{ ")?;

        write!(formatter, "efer: {:#X}, ", self.efer)?;
        write!(formatter, "cr4: {:#X}, ", self.cr4)?;
        write!(formatter, "cr3: {:#X}, ", self.cr3)?;
        write!(formatter, "cr0: {:#X}, ", self.cr0)?;

        write!(formatter, "gdt: {:?}, ", self.gdt_pseudo_descriptor)?;
        write!(formatter, "kernel_code: {:?}, ", self.kernel_code)?;
        write!(formatter, "kernel_data: {:?}, ", self.kernel_data)?;

        write!(formatter, "set_cs_rip_to_64bit: 0p{:04X}, ", self.set_cs_rip_to_64bit)?;
        write!(formatter, "kernel_stack: {}, ", self.kernel_stack)?;
        write!(formatter, "cpu: {}, ", self.cpu)?;
        write!(formatter, "initialized: {} ", self.initialized)?;

        write!(formatter, "}}")
    }

}


/// Переключает Application Processor из [реального режима](https://en.wikipedia.org/wiki/Real_mode) в [64-битный режим](https://en.wikipedia.org/wiki/Long_mode) и выполняет начальную инициализацию процессора.
///
/// Переключение режима происходит напрямую, минуя [32-х битный защищённый режим](https://en.wikipedia.org/wiki/Protected_mode).
/// Все нужные структуры и значения регистров копируются с BSP.
#[allow(named_asm_labels)]
#[cold]
#[naked]
#[unsafe(link_section = ".switch_from_real_mode_to_long_mode")]
#[unsafe(no_mangle)]
extern "C" fn switch_from_real_mode_to_long_mode() -> ! {
    unsafe {
        asm!(
        "
        .code16

        switch_mode_start:

            // Initializing 16-bit mode.
            // Use only 16-bit and 32-bit registers (e.g., AX, EAX, SP, ESP, DS, SS, CR*).
            // 64-bit registers (RAX-R15) are not available here.

            // 1. Initialize page mapping and GDT
            // 2. Reload segment registers with new GDT selectors.
            //    Use `far ret` to reload CS, as it supports variable code selectors.
            //    Note: `far ret` uses 32-bit operation size for far returns in 64-bit mode.

            // Set up data segments
            xor ax, ax
            mov ds, ax
            mov ss, ax

            // Set stack pointer to boot stack
            mov esp, {boot_stack}

            // Set EFER register
            mov ecx, 0xC0000080   // MSR for EFER
            pop eax               // Load EFER value
            wrmsr                 // Write to EFER

            // Set CR4, CR3, CR0 registers
            pop eax
            mov cr4, eax          // Load CR4
            pop eax
            mov cr3, eax          // Load CR3 (page table base)
            pop eax
            mov cr0, eax          // Load CR0

            // Load GDT descriptor
            lgdt [esp]            // Load the GDT descriptor from the stack
            add esp, 6            // Adjust stack to remove GDT descriptor

            // Load data segment selectors from GDT
            pop ax
            mov ds, ax            // Load DS
            mov es, ax            // Load ES
            mov fs, ax            // Load FS
            mov gs, ax            // Load GS
            mov ss, ax            // Load SS

        .code64

            // Far return to switch to 64-bit mode
            // The fields in BootStack are organized to form a far pointer:
            // `kernel_code:set_cs_rip_to_64bit`
            retf                  // Far return to switch to 64-bit mode

        set_cs_rip_to_64bit:

            // Clean up padding and set up arguments for `init_ap`
            add rsp, 4            // Adjust stack for padding

            pop rdi               // First argument for init_ap()
            pop rsi               // Second argument for init_ap()
            pop rsp               // Set stack pointer for the current processor

            mov rax, OFFSET init_ap  // Get address of init_ap
            jmp rax                 // Jump to init_ap

        switch_mode_end:
        ",
        boot_stack = const BOOT_STACK as u32,

        options(noreturn),
        )
    }

}


/// Завершает инициализацию Application Processor.
/// Аргумент `cpu` задаёт структуру [`Cpu`] для запускаемого Application Processor.
/// Через `initialized` запускаемый Application Processor сигнализирует
/// запускающему Bootstrap Processor, что инициализация AP завершена.
#[cfg_attr(not(feature = "conservative-backtraces"), with_sentinel_frame)]
#[cold]
#[inline(never)]
#[unsafe(no_mangle)]
extern "C" fn init_ap(
    cpu: &mut Cpu,            // rdi
    initialized: &AtomicBool, // rsi
) -> ! {
    GDT.lock().load();

    LocalApic::init();

    cpu.set_gs();
    cpu.set_tss();

    IDT.load();
    interrupts::enable();

    syscall::init();

    signal_initialized(initialized);
    // `initialized` is undefined beyond this point since it is on another CPU's stack.
    // And that CPU no longer waits on it.

    info!(cpu = cpu.id(), "report for duty");

    Scheduler::run();
}


/// Возвращает гард, который при своём удалении
/// восстановит исходное содержимое в блоке памяти `original`.
fn save_memory(original: Block<Virt>) -> Result<impl Drop> {
    let original = unsafe { original.try_into_mut_slice::<u8>()? };
    let saved = unsafe { BASE_ADDRESS_SPACE.lock().map_slice_zeroed(original.len(), KERNEL_RW)? };

    saved[..original.len()].clone_from_slice(original);

    Ok(scopeguard::guard((original, saved), |(original, saved)| {
        original.clone_from_slice(&saved[..original.len()]);

        unsafe {
            BASE_ADDRESS_SPACE.lock().unmap_slice(saved).unwrap();
        }
    }))
}


/// Релоцирует код начальной загрузки Application Processor,
/// расположенный в функции [`switch_from_real_mode_to_long_mode()`]
/// в заданный адрес `boot_code_address`.
fn copy_switch_mode_code(boot_code_address: Virt) -> Result<()> {
    let switch_mode_code_address = Virt::from_ptr(switch_from_real_mode_to_long_mode as *const ());
    let switch_mode_code_slice =
        unsafe { switch_mode_code_address.try_into_mut_slice::<u8>(switch_mode_code_size())? };
    let boot_code_slice =
        unsafe { boot_code_address.try_into_mut_slice::<u8>(switch_mode_code_slice.len())? };

    boot_code_slice.clone_from_slice(switch_mode_code_slice);

    Ok(())
}


/// Вычисляет размер кода начальной загрузки Application Processor,
/// расположенного в функции [`switch_from_real_mode_to_long_mode()`].
fn switch_mode_code_size() -> usize {
    let (switch_mode_start, switch_mode_end): (usize, usize);

    unsafe {
        asm!(
        "
            mov {switch_mode_start}, OFFSET switch_mode_start
            mov {switch_mode_end}, OFFSET switch_mode_end
            ",
        switch_mode_start = out(reg) switch_mode_start,
        switch_mode_end = out(reg) switch_mode_end,
        );
    }

    let switch_mode_code_size = switch_mode_end - switch_mode_start;

    assert!(
        switch_mode_code_size + mem::size_of::<BootStack>() <= BOOT_CODE_PLUS_STACK_SIZE,
        "Switch mode code size and BootStack exceed BOOT_CODE_PLUS_STACK_SIZE limit"
    );

    switch_mode_code_size
}



/// Преобразует заданный физический адрес `address` в [`Phys`].
///
/// # Panics
///
/// Паникует, если адрес не влезает в 32 бита и потому не будет доступен в 32-битном режиме.
/// Добавляет `what` в сообщение паники.
fn real_mode_address(address: usize, what: &'static str) -> Phys {
    let address = address
        .try_into()
        .unwrap_or_else(|_| panic!("{} address {:#X} should fit into 32 bit for Application Processors to be able to boot from it", what, address));

    Phys::new_u32(address)
}


/// Сигнализирует запускающему процессору Bootstrap Processor,
/// что Application Processor закончил свою инициализацию.
fn signal_initialized(initialized: &AtomicBool) {
    initialized
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .expect("double or concurrent CPU initialization");
}


/// Ждёт пока запускаемый Application Processor не завершит свою инициализацию.
/// Если этого не произойдёт за отведённый `timeout`, возвращает ошибку [`Error::Timeout`].
fn wait_initialized(initialized: &AtomicBool, timeout: Duration) -> Result<()> {
    let start = time::timer();

    while !start.has_passed(timeout) {
        if initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        hint::spin_loop();
    }

    Err(Timeout)
}