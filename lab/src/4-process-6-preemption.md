## Вытесняющая многозадачность

### Задача 6 --- реализуйте вытеснение пользовательского процесса по прерыванию

Убедитесь, что в запускаемых процессах прерывания разрешены,
то есть установлен флаг
[`RFlags::INTERRUPT_FLAG`](../../doc/ku/process/registers/struct.RFlags.html#associatedconstant.INTERRUPT_FLAG).
См.
[задачу](../../lab/book/4-process-3-user-mode.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-3--%D0%BF%D0%B5%D1%80%D0%B5%D0%BA%D0%BB%D1%8E%D1%87%D0%B5%D0%BD%D0%B8%D0%B5-%D0%BF%D1%80%D0%BE%D1%86%D0%B5%D1%81%D1%81%D0%BE%D1%80%D0%B0-%D0%B2-%D1%80%D0%B5%D0%B6%D0%B8%D0%BC-%D0%BF%D0%BE%D0%BB%D1%8C%D0%B7%D0%BE%D0%B2%D0%B0%D1%82%D0%B5%D0%BB%D1%8F-%D0%B8-%D0%B2%D0%BE%D0%B7%D0%B2%D1%80%D0%B0%D1%82-%D0%B8%D0%B7-%D0%BD%D0%B5%D0%B3%D0%BE)
про переключение процессора в режим пользователя и
[состояние регистров пользовательского процесса](../../lab/book/4-process-3-user-mode.html#%D0%A1%D0%BE%D1%81%D1%82%D0%BE%D1%8F%D0%BD%D0%B8%D0%B5-%D1%80%D0%B5%D0%B3%D0%B8%D1%81%D1%82%D1%80%D0%BE%D0%B2-%D0%BF%D0%BE%D0%BB%D1%8C%D0%B7%D0%BE%D0%B2%D0%B0%D1%82%D0%B5%D0%BB%D1%8C%D1%81%D0%BA%D0%BE%D0%B3%D0%BE-%D0%BF%D1%80%D0%BE%D1%86%D0%B5%D1%81%D1%81%D0%B0).

Реализуйте [метод](../../doc/kernel/process/process/struct.Process.html#method.preempt)

```rust
fn Process::preempt(context: &mut TrapContext)
```

в файле [`kernel/src/process/process.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/process.rs).

Он должен выполнить переключение текущего контекста исполнения `context` на контекст ядра в случае,
если текущий контекст исполняется в режиме пользователя.
Текущий контекст пользователя нужно будет сохранить в структуре
[`Process`](../../doc/kernel/process/process/struct.Process.html).
Но чтобы до неё добраться, нужно захватить спин–блокировку в таблице процессов.
А брать блокировки из обработчиков прерываний не стоит.
Поэтому для начала в самом прерывании сохраним контекст пользователя в промежуточном месте ---
в CPU--локальной структуре
[`kernel::smp::cpu::Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html).
Захватывать блокировку для доступа ней не нужно, так как она не разделяется между разными CPU
и доступ к ней не будет конкурентным.

Метод
[`Process::preempt`](../../doc/kernel/process/process/struct.Process.html#method.preempt)
вызывается из
[функции обработки прерывания от таймера](../../doc/kernel/trap/fn.apic_timer.html)
```rust
extern "x86-interrupt" fn apic_timer(mut context: TrapContext)
```
находящейся в файле
[`kernel/src/trap.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/trap.rs).

[`kernel::trap::TrapContext`](../../doc/kernel/trap/struct.TrapContext.html) ---
обёртка для уже знакомого
[`kernel::process::registers::ModeContext`](../../doc/kernel/process/registers/struct.ModeContext.html).
Она делает запись в него через
[`core::ptr::write_volatile()`](https://doc.rust-lang.org/nightly/core/ptr/fn.write_volatile.html),
чтобы компилятор такую операцию записи не выкинул.
Дело в том, что
[`TrapContext`](../../doc/kernel/trap/struct.TrapContext.html)
формально является изменяемым аргументом
[`apic_timer()`](../../doc/kernel/trap/fn.apic_timer.html),
а не изменяемой ссылкой.
Если бы функция была обычной, запись в `context` сделанную ею, никто снаружи функции бы не заметил.
А значит, компилятор мог бы её выкинуть.
В нашем случае
[`apic_timer()`](../../doc/kernel/trap/fn.apic_timer.html) ---
[`extern "x86-interrupt"`](https://github.com/rust-lang/rust/issues/40180)
и эту запись увидит инструкция процессора
[`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq).
Чтобы предотвратить выкидывание оптимизатором команд записи в `context` и нужен
[`core::ptr::write_volatile()`](https://doc.rust-lang.org/nightly/core/ptr/fn.write_volatile.html).

В [методе](../../doc/kernel/process/process/struct.Process.html#method.enter_user_mode)

```rust
fn Process::enter_user_mode(mut process: SpinlockGuard<Process>) -> bool
```

в файле [`kernel/src/process/process.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/process.rs)
после возвращения из режима пользователя, то есть после возвращения из
[`Registers::switch_to()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_to),
добавьте проверку, что в
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)
есть сохранённый контекст пользователя.
Если его нет, то процесс вернулся из режима пользователя синхронно, например через системный вызов
`exit()` или `sched_yield()`, и дополнительно ничего делать не нужно.
Если же в
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)
сохранён контекст пользователя, то его нужно перенести в структуру
[`Process`](../../doc/kernel/process/process/struct.Process.html).
Для этого, по идентификатору
[`Pid`](../../doc/ku/process/pid/enum.Pid.html)
текущего процесса, получите из
[`Table`](../../doc/kernel/process/struct.Table.html)
его заблокированную спин–блокировкой структуру
[`Process`](../../doc/kernel/process/process/struct.Process.html).
И запишите пользовательский контекст в неё.
Верните из этого метода `true`, если процесс был вытеснен.

Вам пригодятся

- [`TrapContext::is_user_mode()`](../../doc/kernel/trap/struct.TrapContext.html#method.is_user_mode),
- [`TrapContext::get()`](../../doc/kernel/trap/struct.TrapContext.html#method.get),
- [`TrapContext::set()`](../../doc/kernel/trap/struct.TrapContext.html#method.set),
- [`Cpu::kernel_context()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.kernel_context),
- [`Cpu::set_user_context()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.set_user_context),
- [`Cpu::take_user_context()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.take_user_context),
- [`Table::get()`](../../doc/kernel/process/struct.Table.html#method.get),
- [`Registers::set_mode_context()`](../../doc/kernel/process/registers/struct.Registers.html#method.set_mode_context).


### Проверьте себя

После этого заработает тест `preemption()` в файле
[`kernel/tests/4-process-6-preemption.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-6-preemption.rs):

```console
$ (cd kernel; cargo test --test 4-process-6-preemption)
...
4_process_6_preemption::preemption--------------------------
20:39:55 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:39:55 0 I duplicate; address_space = "process" @ 0p7E2_6000
20:39:55 0 I switch to; address_space = "process" @ 0p7E2_6000
20:39:55 0 D extend mapping; block = [0v1000_0000, 0v1000_8234), size 32.551 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:39:56.001 0 D elf loadable program header; file_block = [0v20_3000, 0v20_B234), size 32.551 KiB; memory_block = [0v1000_0000, 0v1000_8234), size 32.551 KiB
20:39:56.033 0 D extend mapping; block = [0v1000_9000, 0v1004_F303), size 280.753 KiB; page_block = [0v1000_9000, 0v1005_0000), size 284.000 KiB
20:39:56.041 0 D elf loadable program header; file_block = [0v20_B240, 0v25_2303), size 284.190 KiB; memory_block = [0v1000_8240, 0v1004_F303), size 284.190 KiB
20:39:56.057 0 D elf loadable program header; file_block = [0v25_2308, 0v25_23D0), size 200 B; memory_block = [0v1004_F308, 0v1004_F3D0), size 200 B
20:39:56.069 0 D extend mapping; block = [0v1005_0000, 0v1005_5F90), size 23.891 KiB; page_block = [0v1005_0000, 0v1005_6000), size 24.000 KiB
20:39:56.079 0 D elf loadable program header; file_block = [0v25_23D0, 0v25_8F68), size 26.898 KiB; memory_block = [0v1004_F3D0, 0v1005_5F90), size 26.938 KiB
20:39:56.119 0 I switch to; address_space = "base" @ 0p1000
20:39:56.123 0 I loaded ELF file; context = { rip: 0v1000_8F60, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.613 MiB; process = { pid: <current>, address_space: "process" @ 0p7E2_6000, { rip: 0v1000_8F60, rsp: 0v7F7F_FFFF_F000 } }
20:39:56.139 0 I allocate; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_6000, { rip: 0v1000_8F60, rsp: 0v7F7F_FFFF_F000 } }; process_count = 1
20:39:56.149 0 I user process page table entry; entry_point = 0v1000_8F60; frame = Frame(32263 @ 0p7E0_7000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:39:56.163 0 D process_frames = 148
20:39:56.167 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:39:56.171 0 D switched to address_space
20:39:56.175 0 D set pid
20:39:56.179 0 D set current_process
20:39:56.183 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1000_8F60, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags: IF } }
20:39:56.269 0 D leaving the user mode; pid = 0:0
20:39:56.273 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1000_82AD, ss:rsp: 0x001B:0v7F7F_FFFF_EFA8, rflags: IF PF }
20:39:56.283 0 D returned from the user space
20:39:56.287 0 I free; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_6000, { rip: 0v1000_82AD, rsp: 0v7F7F_FFFF_EFA8 } }; process_count = 0
20:39:56.295 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 6, unlocks: 6, waits: 0 }
20:39:56.303 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:39:56.309 0 I switch to; address_space = "base" @ 0p1000
20:39:56.315 0 I drop the current address space; address_space = "0:0" @ 0p7E2_6000; switch_to = "base" @ 0p1000
4_process_6_preemption::preemption----------------- [passed]
20:39:56.681 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/process/process.rs | 23 ++++++++++++++++++++---
 1 file changed, 20 insertions(+), 3 deletions(-)
```
