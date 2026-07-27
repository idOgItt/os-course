## Переход в режим пользователя

После того как мы загрузили пользовательский процесс в память,
можно запустить его.
Для это требуется не только перейти на его точку входа, но и переключить процессор в непривилегированный режим --- в
[кольцо защиты](https://en.wikipedia.org/wiki/Protection_ring) 3.
Иначе пользовательский процесс сможет испортить код или данные ядра и других процессов.
Есть [несколько вариантов](https://wiki.osdev.org/Getting_to_Ring_3) сделать это.
Воспользуемся инструкцией
[`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq).


### Состояние регистров пользовательского процесса

Состояние регистров пользовательского процесса хранится в структуре
[`kernel::process::registers::Registers`](../../doc/kernel/process/registers/struct.Registers.html).
Для использования
[`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq)
нам понадобится её поле
[`Registers::user_context`](../../doc/kernel/process/registers/struct.Registers.html#structfield.user_context).
Оно является структурой
[`kernel::process::registers::ModeContext`](../../doc/kernel/process/registers/struct.ModeContext.html):
```rust
#[repr(C)]
pub(crate) struct ModeContext {
    rip: Virt,
    cs: usize,
    rflags: RFlags,
    rsp: Virt,
    ss: usize,
}
```
Которая имеет ровно такое представление в памяти, какого требует инструкция
[`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq).
Если регистр `RSP` указывает на адрес
[`ModeContext`](../../doc/kernel/process/registers/struct.ModeContext.html)
в момент выполнения инструкции
[`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq),
процессор загрузит

- поле [`ModeContext::rip`](../../doc/kernel/process/registers/struct.ModeContext.html#structfield.rip) в регистр адреса команды `RIP`,
- поле [`ModeContext::cs`](../../doc/kernel/process/registers/struct.ModeContext.html#structfield.cs) в регистр сегмента кода `CS`,
- поле [`ModeContext::rflags`](../../doc/kernel/process/registers/struct.ModeContext.html#structfield.rflags) в регистр флагов `RFLAGS`,
- поле [`ModeContext::rsp`](../../doc/kernel/process/registers/struct.ModeContext.html#structfield.rsp) в регистр адреса стека `RSP`,
- поле [`ModeContext::ss`](../../doc/kernel/process/registers/struct.ModeContext.html#structfield.ss) в регистр сегмента стека `SS`.

Слово mode в [`ModeContext`](../../doc/kernel/process/registers/struct.ModeContext.html)
символизирует тот факт, что эта структура позволяет переключаться между режимами пользователя и ядра.

С помощью метода
[`ModeContext::user_context()`](../../doc/kernel/process/registers/struct.ModeContext.html#method.user_context)
заполним эти поля так:

- Поле [`ModeContext::rip`](../../doc/kernel/process/registers/struct.ModeContext.html#structfield.rip) будет содержать точку входа в программу пользователя, которую возвращает функция [`ku::process::elf::load()`](../../doc/ku/process/elf/fn.load.html).
- Поле [`ModeContext::cs`](../../doc/kernel/process/registers/struct.ModeContext.html#structfield.cs) --- селектор кода пользователя, который возвращает метод [`kernel::memory::gdt::Gdt::user_code()`](../../doc/kernel/memory/gdt/struct.SmpGdt.html#method.user_code).
- Поле [`ModeContext::rflags`](../../doc/kernel/process/registers/struct.ModeContext.html#structfield.rflags) будет содержать единственный включённый флаг --- [`RFlags::INTERRUPT_FLAG`](../../doc/ku/process/registers/struct.RFlags.html#associatedconstant.INTERRUPT_FLAG). Это требуется чтобы по прерыванию, например, от таймера, процессор вернулся в ядро. И пользовательский код не смог его монополизировать.
- Поле [`ModeContext::rsp`](../../doc/kernel/process/registers/struct.ModeContext.html#structfield.rsp) будет содержать адрес конца стека пользователя --- в [x86-64](https://en.wikipedia.org/wiki/X86-64) стек растёт от старших адресов к младшим. Стек для пользовательского процесса выделяет функция [`kernel::process::create()`](../../doc/kernel/process/fn.create.html) с помощью метода [`kernel::memory::stack::Stack::new()`](../../doc/kernel/memory/stack/struct.Stack.html#method.new). Метод [`Stack::new()`](../../doc/kernel/memory/stack/struct.Stack.html#method.new) аллоцирует [`kernel::memory::stack::Stack::STACK_SIZE`](../../doc/kernel/memory/stack/struct.Stack.html#associatedconstant.STACK_SIZE) байт памяти и запрещает доступ к младшим [`kernel::memory::stack::Stack::GUARD_ZONE_SIZE`](../../doc/kernel/memory/stack/struct.Stack.html#associatedconstant.GUARD_ZONE_SIZE) байтам из них, чтобы отлавливать переполнение стека. Начальное значение указателя стека возвращает метод [`Stack::pointer()`](../../doc/kernel/memory/stack/struct.Stack.html#method.pointer).
- Поле [`ModeContext::ss`](../../doc/kernel/process/registers/struct.ModeContext.html#structfield.ss) будет содержать селектор данных пользователя, который возвращает метод [`kernel::memory::gdt::Gdt::user_data()`](../../doc/kernel/memory/gdt/struct.SmpGdt.html#method.user_data).

Конкретные значения остальных полей структуры
[`Registers`](../../doc/kernel/process/registers/struct.Registers.html),
в которых хранятся регистры общего назначения кроме `RSP`, нас пока не интересуют.


### Задача 3 --- переключение процессора в режим пользователя и возврат из него

Само переключение выполняет [метод](../../doc/kernel/process/registers/struct.Registers.html#method.switch_to)

```rust
unsafe fn Registers::switch_to(registers: *const Registers)
```

в файле
[`kernel/src/process/registers.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/registers.rs).

Для его реализации придётся использовать ассемблер, например прибегнуть к макросу
[`asm!()`](https://doc.rust-lang.org/core/arch/macro.asm.html).
Документацию на который можно посмотреть в
[Rust By Example](https://doc.rust-lang.org/nightly/rust-by-example/unsafe/asm.html) и
[The Rust Reference](https://doc.rust-lang.org/nightly/reference/inline-assembly.html).
Кроме того, за счёт `#![feature(asm_const)]` нам доступна
[передача констант в параметрах макроса `asm!()`](https://doc.rust-lang.org/unstable-book/language-features/asm-const.html).

Мы не будем явно сохранять контекст ядра.
Пусть это сделает за нас компилятор.
Мы просто укажем ему, что испортили состояние всех регистров, с помощью конструкции `lateout(...) _`:

```rust
asm!(
    "
    ...
    ",
    ...
    lateout("rax") _,
    ...
);
```

Есть надежда, что компилятор тогда не будет сохранять
[callee-saved](https://en.wikipedia.org/wiki/X86_calling_conventions#Callee-saved_%28non-volatile%29_registers)
регистры.
Единственное что, он не согласится на порчу регистров `RBX` и `RBP`.
Эти регистры мы должны будем сохранить на стеке ядра и восстановить вручную.

Таким образом для переключения в режим пользователя метод
[`Registers::switch_to()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_to)
должен:

- Сохранить регистры `RBX` и `RBP` в стеке ядра.
- Сохранить адрес `registers` в стеке.
- Записать текущее состояние стека ядра по адресу `GS:offset`, где `offset` --- смещение `RSP` ядра в структуре [`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html) текущего процессора. Для этого смещения есть константа [`kernel::smp::cpu::KERNEL_RSP_OFFSET_IN_CPU`](../../doc/kernel/smp/cpu/constant.KERNEL_RSP_OFFSET_IN_CPU.html). Обратите внимание, что в ассемблере нужно использовать запись `GS:offset`. Высокоуровневые методы вроде [`Cpu::get()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.get) доступны только для Rust--кода --- до переключения стека невозможно корректно позвать Rust--функцию.
- Запретить прерывания инструкцией [`cli`](https://www.felixcloutier.com/x86/cli) процессора. Дальше мы будем переключать стек и не хотим чтобы прерывание записало адрес возврата в неподходящее место, --- не на стек ядра.
- Переключить стек, то есть регистр `RSP`, на заданный методу [`Registers::switch_to()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_to) на вход адрес `registers`.
- Восстановить регистры общего назначения с `RAX` по `R15` (кроме `RSP`) из стека. То есть, на самом деле из структуры [`Registers`](../../doc/kernel/process/registers/struct.Registers.html). На которую он переключил стек.
- Выполнить инструкцию [`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq), чтобы перейти в контекст пользователя, заданный полем [`Registers::user_context`](../../doc/kernel/process/registers/struct.Registers.html#structfield.user_context). Обратите внимание на суффикс `q` у [`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq). Если его не указать, ассемблер сгенерирует машинный код для другого режима работы процессора и до некоторого момента это будет не заметно, так как делать он будет почти то же самое. А потом в неожиданный момент всё сломается и найти такую ошибку будет тяжело.

Последующие инструкции метода
[`Registers::switch_to()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_to)
будут выполняться при возвращении в режим ядра.
Начало этой части метода
[`Registers::switch_to()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_to)
выделено меткой `store_user_mode_context`.
Парный метод
[`Registers::switch_from()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_from)
просто прыгает на эту метку:

```rust
pub(super) unsafe extern "C" fn switch_from() -> ! {
    asm!(
        "jmp store_user_mode_context",
        options(noreturn),
    );
}
```

Когда пользовательский код выполнит системный вызов и процессор окажется в режиме ядра, мы сможем вызвать
[`Registers::switch_from()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_from)
для прекращения исполнения кода пользователя и возврата ровно в тот контекст ядра, который выполнил вызов
[`Registers::switch_to()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_to).
Например, это может быть контекст планировщика, который продолжит свой цикл исполнения, выберет следующий процесс и переключится уже в него.

Итак, после метки `store_user_mode_context` метод
[`Registers::switch_to()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_to)
должен:

- Вспомнить состояние стека ядра. Его можно прочитать по логическому адресу `GS:offset`, где `offset` --- смещение `RSP` ядра в структуре [`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html) текущего процессора. Для этого смещения есть константа [`kernel::smp::cpu::KERNEL_RSP_OFFSET_IN_CPU`](../../doc/kernel/smp/cpu/constant.KERNEL_RSP_OFFSET_IN_CPU.html). Не пытайтесь прочитать из `GS` инструкцией [`rdmsr`](https://www.felixcloutier.com/x86/rdmsr). Она испортит регистры пространства пользователя до того как у нас появится место куда их нужно сохранить. Поэтому используйте конструкции подобные
```rust
asm!(
    "
    ...
    mov gs:[{rsp_offset}], ...
    ...
    mov ..., gs:[{rsp_offset}]
    ...
    ",
    rsp_offset = const KERNEL_RSP_OFFSET_IN_CPU,
    ...
);
```
- Восстановить из стека ядра сохранённый адрес `registers`.
- Переключить стек на этот адрес плюс суммарный размер регистров общего назначения `user_registers_size = const mem::size_of::<Registers>() - mem::size_of::<ModeContext>()`.
- Записать в стек, то есть на самом деле в заданную на вход структуру [`Registers`](../../doc/kernel/process/registers/struct.Registers.html), регистры общего назначения с `R15` по `RAX`. В обратном порядке, так как инструкции `push` и `pop` должны образовывать правильную скобочную последовательность с именами регистров в качестве типов скобок.
- Переключить регистр `RSP` на стек ядра.
- Разрешить прерывания инструкцией [`sti`](https://www.felixcloutier.com/x86/sti) процессора. Мы вернулись на стек ядра и теперь безопасно получить прерывание, что приведёт к записи адреса возврата на стек.
- Вытолкнуть из стека ядра сохранённый там адрес `registers`. Неважно куда, он больше не понадобится.
- Восстановить регистры `RBX` и `RBP`, помня про правильную скобочную последовательность.

Не стоит смешивать в одном макросе `asm!()` автоматическое выделение регистра компилятором --- `in(reg)`, ---
и явное использование фиксированного регистра в какой-либо инструкции вами самостоятельно.
Компилятор не парсит ассемблер и не знает что вы какие-то регистры в нём явно используете.
И, аллоцируя регистр в `in(reg)`, не может проверить коллизии.
А при коллизии вас может ждать сложная и продолжительная отладка.
Ещё один источник проблем ---
модификация регистра, который вы указали в `in(...)`, но не указали в `lateout(...)`.
Из [документации](https://doc.rust-lang.org/nightly/reference/inline-assembly.html#operand-type):
> `in(<reg>) <expr>`
>
> - ...
> - The allocated register must contain the same value at the end of the asm code (except if a `lateout` is allocated to the same register).

При желании вы можете отступить от предложенной схемы и реализовать любой другой работающий вариант.
Например, регистры общего назначения можно сохранять и восстанавливать командами
[`mov`](https://www.felixcloutier.com/x86/mov).

> Вместо запрета прерываний на время манипуляций со стеком ядра можно было бы
> выделить для всех прерываний отдельный стек.
> Такой стек должен был бы быть собственным у каждого процессора.


### Логирование в режиме пользователя

Учтите, что в режиме пользователя структурированное логирование макросами библиотеки
[tracing](https://docs.rs/tracing/) ---
`info!()`, `debug!()` и т.д., пока работать не будет.
В следующей задаче мы сделаем ему временную замену --- системный вызов `log_value()`.
А чтобы заработало привычное структурированное логирование, нужно сделать
[первую часть пятой лабораторки](../../lab/book/5-um-1-ring-buffer.html).


### Проверьте себя

Запустите тест `4-process-3-user-mode` из файла
[`kernel/tests/4-process-3-user-mode.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-3-user-mode.rs):

```console
$ (cd kernel; cargo test --test 4-process-3-user-mode)
...
4_process_3_user_mode::user_context_saved-------------------
20:37:19 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:37:19 0 I duplicate; address_space = "process" @ 0p7E9_9000
20:37:19 0 I switch to; address_space = "process" @ 0p7E9_9000
20:37:19 0 D extend mapping; block = [0v1000_0000, 0v1000_8974), size 34.363 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:37:19 0 D elf loadable program header; file_block = [0v20_3000, 0v20_B974), size 34.363 KiB; memory_block = [0v1000_0000, 0v1000_8974), size 34.363 KiB
20:37:19 0 D extend mapping; block = [0v1000_9000, 0v1005_3B4B), size 298.823 KiB; page_block = [0v1000_9000, 0v1005_4000), size 300.000 KiB
20:37:19 0 D elf loadable program header; file_block = [0v20_B980, 0v25_6B4B), size 300.448 KiB; memory_block = [0v1000_8980, 0v1005_3B4B), size 300.448 KiB
20:37:19 0 D elf loadable program header; file_block = [0v25_6B50, 0v25_6C18), size 200 B; memory_block = [0v1005_3B50, 0v1005_3C18), size 200 B
20:37:19 0 D extend mapping; block = [0v1005_4000, 0v1005_ACA0), size 27.156 KiB; page_block = [0v1005_4000, 0v1005_B000), size 28.000 KiB
20:37:19 0 D elf loadable program header; file_block = [0v25_6C18, 0v25_DC78), size 28.094 KiB; memory_block = [0v1005_3C18, 0v1005_ACA0), size 28.133 KiB
20:37:19 0 I switch to; address_space = "base" @ 0p1000
20:37:19 0 I loaded ELF file; context = { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.659 MiB; process = { pid: <current>, address_space: "process" @ 0p7E9_9000, { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 } }
20:37:19 0 I user process page table entry; entry_point = 0v1000_9A40; frame = Frame(32320 @ 0p7E4_0000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:37:19 0 D process_frames = 153
20:37:19 0 I switch to; address_space = "process" @ 0p7E9_9000
20:37:19 0 D switched to address_space
20:37:19 0 D set pid
20:37:19 0 D set current_process
20:37:19 0 D entering the user mode; pid = <current>; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1000_9A40, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags:  } }
20:37:19 0 D leaving the user mode; pid = <current>
20:37:19 0 D user_registers = [77701, 77702, 77703, 77704, 77705, 77706, 77707, 77708, 77709, 77710, 77711, 77712, 77713, 77714, 77715]
20:37:19 0 D dropping; spinlock = kernel/tests/4-process-3-user-mode.rs:53:19; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:37:19 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 1, unlocks: 1, waits: 0 }
20:37:19 0 I switch to; address_space = "base" @ 0p1000
20:37:19 0 I drop the current address space; address_space = "process" @ 0p7E9_9000; switch_to = "base" @ 0p1000
4_process_3_user_mode::user_context_saved---------- [passed]

4_process_3_user_mode::user_mode_page_fault-----------------
20:37:20.223 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:37:20.231 0 I duplicate; address_space = "process" @ 0p7E9_9000
20:37:20.235 0 I switch to; address_space = "process" @ 0p7E9_9000
20:37:20.241 0 D extend mapping; block = [0v1000_0000, 0v1000_8974), size 34.363 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:37:20.249 0 D elf loadable program header; file_block = [0v20_3000, 0v20_B974), size 34.363 KiB; memory_block = [0v1000_0000, 0v1000_8974), size 34.363 KiB
20:37:20.281 0 D extend mapping; block = [0v1000_9000, 0v1005_3B4B), size 298.823 KiB; page_block = [0v1000_9000, 0v1005_4000), size 300.000 KiB
20:37:20.289 0 D elf loadable program header; file_block = [0v20_B980, 0v25_6B4B), size 300.448 KiB; memory_block = [0v1000_8980, 0v1005_3B4B), size 300.448 KiB
20:37:20.305 0 D elf loadable program header; file_block = [0v25_6B50, 0v25_6C18), size 200 B; memory_block = [0v1005_3B50, 0v1005_3C18), size 200 B
20:37:20.313 0 D extend mapping; block = [0v1005_4000, 0v1005_ACA0), size 27.156 KiB; page_block = [0v1005_4000, 0v1005_B000), size 28.000 KiB
20:37:20.321 0 D elf loadable program header; file_block = [0v25_6C18, 0v25_DC78), size 28.094 KiB; memory_block = [0v1005_3C18, 0v1005_ACA0), size 28.133 KiB
20:37:20.363 0 I switch to; address_space = "base" @ 0p1000
20:37:20.365 0 I loaded ELF file; context = { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.659 MiB; process = { pid: <current>, address_space: "process" @ 0p7E9_9000, { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 } }
20:37:20.377 0 I user process page table entry; entry_point = 0v1000_9A40; frame = Frame(32223 @ 0p7DD_F000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:37:20.387 0 D process_frames = 153
20:37:20.391 0 I switch to; address_space = "process" @ 0p7E9_9000
20:37:20.395 0 D switched to address_space
20:37:20.397 0 D set pid
20:37:20.401 0 D set current_process
20:37:20.403 0 D entering the user mode; pid = <current>; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1000_9A40, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags:  } }
20:37:20.515 0 D leaving the user mode; pid = <current>
20:37:20.519 0 D dropping; spinlock = kernel/tests/4-process-3-user-mode.rs:33:19; stats = Stats { failures: 0, locks: 2, unlocks: 2, waits: 0 }
20:37:20.525 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 1, unlocks: 1, waits: 0 }
20:37:20.533 0 I switch to; address_space = "base" @ 0p1000
20:37:20.535 0 I drop the current address space; address_space = "process" @ 0p7E9_9000; switch_to = "base" @ 0p1000
4_process_3_user_mode::user_mode_page_fault-------- [passed]
20:37:20.817 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```

Если вы получаете Page Fault в ядре при доступе к адресу `0vFFFFFFFFFFFFFFF8`:

```
19:53:29 0 E kernel mode trap; trap = "Page Fault"; number = 14; info = { code: 0b10 = non-present page | write | kernel, address: 0vFFFFFFFFFFFFFFF8 }; context = { mode: kernel, cs:rip: 0x0008:0v76FF50, ss:rsp: 0x0010:0v0, rflags: IF }
panicked at 'kernel mode trap #14 - Page Fault
ModeContext { rip: Virt(0v76FF50), cs: 8, rflags: RFlags(514), rsp: Virt(0v0), ss: 16 }', kernel/src/trap.rs:408:13
```

Это означает, что прерывания разрешаются в неправильный момент, когда ядро ещё не переключилось на свой стек.
В этом логе видно, что `RSP` нулевой --- `ss:rsp: 0x0010:0v0`.


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/process/registers.rs | 103 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++----
 1 file changed, 99 insertions(+), 4 deletions(-)
```
