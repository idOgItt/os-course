## Round-robin планировщик

Планировщик процессов
[`kernel::process::scheduler::Scheduler`](../../doc/kernel/process/scheduler/struct.Scheduler.html)
расположен в файле
[`kernel/src/process/scheduler.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/scheduler.rs)
и реализует простейшее
[циклическое исполнение процессов](https://en.wikipedia.org/wiki/Round-robin_scheduling).
Он содержит очередь процессов
[`Scheduler::queue`](../../doc/kernel/process/scheduler/struct.Scheduler.html#structfield.queue)
такой же ёмкости, как таблица процессов.
И, так же как и таблица процессов, является [синглтоном](https://en.wikipedia.org/wiki/Singleton_pattern)
[`static ref SCHEDULER: Mutex<Scheduler>`](../../doc/kernel/process/scheduler/struct.SCHEDULER.html).


### Задача 7 --- планировщик

#### Реализуйте методы планировщика

- [`fn Scheduler::enqueue(pid: Pid)`](../../doc/kernel/process/scheduler/struct.Scheduler.html#method.enqueue) ставит процесс, заданный идентификатором `pid`, в очередь исполнения.
- [`fn Scheduler::dequeue() -> Option<Pid>`](../../doc/kernel/process/scheduler/struct.Scheduler.html#method.dequeue) достаёт из очереди первый процесс.
- [`fn Scheduler::run_one() -> bool`](../../doc/kernel/process/scheduler/struct.Scheduler.html#method.run_one) выполняет один цикл работы --- берёт первый процесс из очереди и исполняет его пользовательский код. Если метод [`Process::enter_user_mode()`](../../doc/kernel/process/process/struct.Process.html#method.enter_user_mode) вернёт `true`, то есть процесс был снят с CPU принудительно по прерыванию таймера, [`Scheduler::run_one()`](../../doc/kernel/process/scheduler/struct.Scheduler.html#method.run_one) перепланирует исполнение процесса, ставя его в конец очереди. Метод [`Scheduler::run_one()`](../../doc/kernel/process/scheduler/struct.Scheduler.html#method.run_one) возвращает `true` если в очереди на исполнение нашёлся хотя бы один процесс. Он должен корректно обрабатывать ситуацию, когда `pid` есть в очереди планирования, но соответствующего процесса уже нет в [`kernel::process::Table`](../../doc/kernel/process/struct.Table.html).

Метод
[`fn Scheduler::run() -> !`](../../doc/kernel/process/scheduler/struct.Scheduler.html#method.run)
уже реализован.
Он в вечном цикле выполняет
[`Scheduler::run_one()`](../../doc/kernel/process/scheduler/struct.Scheduler.html#method.run_one).
Если в очереди на исполнение процессов не нашлось,
то он выключает процессор до прихода следующего прерывания, самое долгое --- до следующего тика таймера

```rust
extern "x86-interrupt" fn apic_timer(mut context: TrapContext)
```

Для этого служит специальная инструкция
[`hlt`](https://www.felixcloutier.com/x86/hlt),
которая зовётся через функцию
[`x86_64::instructions::hlt()`](../../doc/x86_64/instructions/fn.hlt.html).


#### Системный вызов [`kernel::process::syscall::sched_yield()`](../../doc/kernel/process/syscall/fn.sched_yield.html)

Добавьте в файл
[`kernel/src/process/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/syscall.rs)
реализацию системного вызова
[`kernel::process::syscall::sched_yield()`](../../doc/kernel/process/syscall/fn.sched_yield.html)
с номером
[`ku::process::syscall::Syscall::SchedYield`](../../doc/ku/process/syscall/enum.Syscall.html#variant.SchedYield).
Он должен перепланировать процесс в конец очереди и забрать у него CPU функцией
[`kernel::process::process::Process::sched_yield()`](../../doc/kernel/process/process/struct.Process.html#method.sched_yield),
которая вернёт управление в другой контекст ядра.
А именно, в контекст из которого была вызвана функция
[`kernel::process::process::Process::enter_user_mode()`](../../doc/kernel/process/process/struct.Process.html#method.enter_user_mode).
То есть, управление вернётся в цикл планировщика.

Функция
[`kernel::process::syscall::sched_yield()`](../../doc/kernel/process/syscall/fn.sched_yield.html)
не вернётся в диспетчер системных вызовов
[`kernel::process::syscall::syscall()`](../../doc/kernel/process/syscall/fn.syscall.html)
и далее в возврат из системного вызова
[`kernel::process::syscall::sysret()`](../../doc/kernel/process/syscall/fn.sysret.html).
Но, тем не менее, когда-нибудь управление нужно будет передать вызывающему процессу.
Это сделает планировщик через цепочку
[`kernel::process::process::Process::enter_user_mode()`](../../doc/kernel/process/process/struct.Process.html#method.enter_user_mode) ->
[`kernel::process::registers::Registers::switch_to()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_to) ->
[`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq).
И для
[`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq)
нужно предоставить правильный контекст пользователя, в который нужно будет переключиться.
Это ровно тот контекст пользователя, который есть у
[`kernel::process::syscall::sched_yield()`](../../doc/kernel/process/syscall/fn.sched_yield.html).
И который системные вызовы обычно передают в
[`kernel::process::syscall::sysret()`](../../doc/kernel/process/syscall/fn.sysret.html).
А в
[`kernel::process::syscall::sched_yield()`](../../doc/kernel/process/syscall/fn.sched_yield.html)
этот контекст пользователя нужно явно сохранить методом
[`kernel::process::process::Process::set_context()`](../../doc/kernel/process/process/struct.Process.html#method.set_context).
Флаги и сегментные регистры тут сохранять не требуется.
Флаги должен сохранить пользовательский код, если они ему нужны.
Так как это syscall, то есть пользователь готов к тому что регистры, и в том числе флаги, будут изменены.
А сегментные регистры не меняются на протяжении жизни процесса.

Результат системного вызова
[`kernel::process::syscall::sched_yield()`](../../doc/kernel/process/syscall/fn.sched_yield.html)
не может вернуть так, как это делают другие системные вызовы, --- через функцию
[`kernel::process::syscall::sysret()`](../../doc/kernel/process/syscall/fn.sysret.html).
Но может сохранить в контексте процесса --- в
[`kernel::process::registers::Registers`](../../doc/kernel/process/registers/struct.Registers.html).
Либо вы можете не возвращать из этого системного вызова никакого осмысленного результата,
так как ошибок возникнуть не может.
Но тогда учтите, что возвращаемый этим системным вызовом результат нужно проигнорировать
на стороне пользователя.


### Проверьте себя

Теперь должны заработать тесты `syscall_sched_yield()` и `scheduler()` в файле
[`kernel/tests/4-process-7-scheduler.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-7-scheduler.rs):

```console
$ (cd kernel; cargo test --test 4-process-7-scheduler)
...
4_process_7_scheduler::scheduler----------------------------
20:40:09.385 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:40:09.399 0 I duplicate; address_space = "process" @ 0p7E2_6000
20:40:09.405 0 I switch to; address_space = "process" @ 0p7E2_6000
20:40:09.417 0 D extend mapping; block = [0v1000_0000, 0v1000_8974), size 34.363 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:40:09.431 0 D elf loadable program header; file_block = [0v8A_1000, 0v8A_9974), size 34.363 KiB; memory_block = [0v1000_0000, 0v1000_8974), size 34.363 KiB
20:40:09.467 0 D extend mapping; block = [0v1000_9000, 0v1005_3B4B), size 298.823 KiB; page_block = [0v1000_9000, 0v1005_4000), size 300.000 KiB
20:40:09.477 0 D elf loadable program header; file_block = [0v8A_9980, 0v8F_4B4B), size 300.448 KiB; memory_block = [0v1000_8980, 0v1005_3B4B), size 300.448 KiB
20:40:09.495 0 D elf loadable program header; file_block = [0v8F_4B50, 0v8F_4C18), size 200 B; memory_block = [0v1005_3B50, 0v1005_3C18), size 200 B
20:40:09.505 0 D extend mapping; block = [0v1005_4000, 0v1005_ACA0), size 27.156 KiB; page_block = [0v1005_4000, 0v1005_B000), size 28.000 KiB
20:40:09.515 0 D elf loadable program header; file_block = [0v8F_4C18, 0v8F_BC78), size 28.094 KiB; memory_block = [0v1005_3C18, 0v1005_ACA0), size 28.133 KiB
20:40:09.551 0 I switch to; address_space = "base" @ 0p1000
20:40:09.557 0 I loaded ELF file; context = { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.661 MiB; process = { pid: <current>, address_space: "process" @ 0p7E2_6000, { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 } }
20:40:09.575 0 I allocate; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_6000, { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 } }; process_count = 1
20:40:09.589 0 I user process page table entry; entry_point = 0v1000_9A40; frame = Frame(32255 @ 0p7DF_F000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:40:09.613 0 D process_frames = 160
20:40:10.265 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:40:10.275 0 I duplicate; address_space = "process" @ 0p7D8_6000
20:40:10.281 0 I switch to; address_space = "process" @ 0p7D8_6000
20:40:10.287 0 D extend mapping; block = [0v1000_0000, 0v1000_8974), size 34.363 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:40:10.299 0 D elf loadable program header; file_block = [0vF4_B000, 0vF5_3974), size 34.363 KiB; memory_block = [0v1000_0000, 0v1000_8974), size 34.363 KiB
20:40:10.329 0 D extend mapping; block = [0v1000_9000, 0v1005_3B4B), size 298.823 KiB; page_block = [0v1000_9000, 0v1005_4000), size 300.000 KiB
20:40:10.339 0 D elf loadable program header; file_block = [0vF5_3980, 0vF9_EB4B), size 300.448 KiB; memory_block = [0v1000_8980, 0v1005_3B4B), size 300.448 KiB
20:40:10.355 0 D elf loadable program header; file_block = [0vF9_EB50, 0vF9_EC18), size 200 B; memory_block = [0v1005_3B50, 0v1005_3C18), size 200 B
20:40:10.367 0 D extend mapping; block = [0v1005_4000, 0v1005_ACA0), size 27.156 KiB; page_block = [0v1005_4000, 0v1005_B000), size 28.000 KiB
20:40:10.377 0 D elf loadable program header; file_block = [0vF9_EC18, 0vFA_5C78), size 28.094 KiB; memory_block = [0v1005_3C18, 0v1005_ACA0), size 28.133 KiB
20:40:10.407 0 I switch to; address_space = "base" @ 0p1000
20:40:10.413 0 I loaded ELF file; context = { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.659 MiB; process = { pid: <current>, address_space: "process" @ 0p7D8_6000, { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 } }
20:40:10.427 0 I allocate; slot = Process { pid: 1:0, address_space: "1:0" @ 0p7D8_6000, { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 } }; process_count = 2
20:40:10.437 0 I user process page table entry; entry_point = 0v1000_9A40; frame = Frame(32095 @ 0p7D5_F000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:40:10.449 0 D process_frames = 160
20:40:10.455 0 I dequeue; pid = Some(0:0)
20:40:10.459 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:40:10.465 0 D switched to address_space
20:40:10.471 0 D set pid
20:40:10.475 0 D set current_process
20:40:10.481 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1000_9A40, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags: IF } }
20:40:10.597 0 D leaving the user mode; pid = 0:0
20:40:10.601 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1001_7B8B, ss:rsp: 0x001B:0v7F7F_FFFF_EE98, rflags: IF ZF PF }
20:40:10.615 0 I returned
20:40:10.619 0 I dequeue; pid = Some(1:0)
20:40:10.623 0 I switch to; address_space = "1:0" @ 0p7D8_6000
20:40:10.629 0 D switched to address_space
20:40:10.633 0 D set pid
20:40:10.637 0 D set current_process
20:40:10.641 0 D entering the user mode; pid = 1:0; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1000_9A40, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags: IF } }
20:40:10.695 0 D leaving the user mode; pid = 1:0
20:40:10.699 0 I the process was preempted; pid = 1:0; user_context = { mode: user, cs:rip: 0x0023:0v1001_5206, ss:rsp: 0x001B:0v7F7F_FFFF_EEB8, rflags: IF }
20:40:10.709 0 I returned
20:40:10.713 0 I dequeue; pid = Some(0:0)
20:40:10.717 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:40:10.721 0 D switched to address_space
20:40:10.727 0 D set pid
20:40:10.729 0 D set current_process
20:40:10.733 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1211F600, rdi: 0x0, rsi: 0x93F98C2A4, { mode: user, cs:rip: 0x0023:0v1001_7B8B, ss:rsp: 0x001B:0v7F7F_FFFF_EE98, rflags: IF ZF PF } }
20:40:10.751 0 I free; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_6000, { rip: 0v1001_7B8B, rsp: 0v7F7F_FFFF_EE98 } }; process_count = 1
20:40:10.761 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 7, unlocks: 7, waits: 0 }
20:40:10.771 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:40:10.779 0 I switch to; address_space = "base" @ 0p1000
20:40:10.785 0 I drop the current address space; address_space = "0:0" @ 0p7E2_6000; switch_to = "base" @ 0p1000
20:40:12.113 0 I syscall = "exit"; pid = 0:0; code = 3141592653589793238; reason = Err(TryFromPrimitiveError { number: 3141592653589793238 })
20:40:12.121 0 D leaving the user mode; pid = 0:0
20:40:12.127 0 I dequeue; pid = Some(1:0)
20:40:12.131 0 I switch to; address_space = "1:0" @ 0p7D8_6000
20:40:12.135 0 D switched to address_space
20:40:12.139 0 D set pid
20:40:12.143 0 D set current_process
20:40:12.147 0 D entering the user mode; pid = 1:0; registers = { rax: 0x1, rdi: 0x7F7FFFFFEF58, rsi: 0x7F7FFFFFEF30, { mode: user, cs:rip: 0x0023:0v1001_5206, ss:rsp: 0x001B:0v7F7F_FFFF_EEB8, rflags: IF } }
20:40:12.169 0 I user mode trap; trap = "Page Fault"; number = 14; info = { address: 0v0, code: 0b100 = non-present page | read | user }; context = { mode: user, cs:rip: 0x0023:0v1000_8D94, ss:rsp: 0x001B:0v0, rflags: IF ZF PF }; pid = 1:0
20:40:12.193 0 I free; slot = Process { pid: 1:0, address_space: "1:0" @ 0p7D8_6000, { rip: 0v1001_5206, rsp: 0v7F7F_FFFF_EEB8 } }; process_count = 0
20:40:12.209 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 7, unlocks: 7, waits: 0 }
20:40:12.219 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:40:12.229 0 I switch to; address_space = "base" @ 0p1000
20:40:12.233 0 I drop the current address space; address_space = "1:0" @ 0p7D8_6000; switch_to = "base" @ 0p1000
20:40:12.811 0 D leaving the user mode; pid = 1:0
20:40:12.815 0 I dequeue; pid = None
4_process_7_scheduler::scheduler------------------- [passed]

4_process_7_scheduler::syscall_sched_yield------------------
20:40:13.479 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:40:13.487 0 I duplicate; address_space = "process" @ 0p7D8_6000
20:40:13.491 0 I switch to; address_space = "process" @ 0p7D8_6000
20:40:13.499 0 D extend mapping; block = [0v1000_0000, 0v1000_8234), size 32.551 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:40:13.507 0 D elf loadable program header; file_block = [0v20_3000, 0v20_B234), size 32.551 KiB; memory_block = [0v1000_0000, 0v1000_8234), size 32.551 KiB
20:40:13.539 0 D extend mapping; block = [0v1000_9000, 0v1004_F293), size 280.644 KiB; page_block = [0v1000_9000, 0v1005_0000), size 284.000 KiB
20:40:13.549 0 D elf loadable program header; file_block = [0v20_B240, 0v25_2293), size 284.081 KiB; memory_block = [0v1000_8240, 0v1004_F293), size 284.081 KiB
20:40:13.565 0 D elf loadable program header; file_block = [0v25_2298, 0v25_2360), size 200 B; memory_block = [0v1004_F298, 0v1004_F360), size 200 B
20:40:13.575 0 D extend mapping; block = [0v1005_0000, 0v1005_5F20), size 23.781 KiB; page_block = [0v1005_0000, 0v1005_6000), size 24.000 KiB
20:40:13.585 0 D elf loadable program header; file_block = [0v25_2360, 0v25_8EF8), size 26.898 KiB; memory_block = [0v1004_F360, 0v1005_5F20), size 26.938 KiB
20:40:13.625 0 I switch to; address_space = "base" @ 0p1000
20:40:13.629 0 I loaded ELF file; context = { rip: 0v1000_8EF0, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.613 MiB; process = { pid: <current>, address_space: "process" @ 0p7D8_6000, { rip: 0v1000_8EF0, rsp: 0v7F7F_FFFF_F000 } }
20:40:13.641 0 I allocate; slot = Process { pid: 1:1, address_space: "1:1" @ 0p7D8_6000, { rip: 0v1000_8EF0, rsp: 0v7F7F_FFFF_F000 } }; process_count = 1
20:40:13.651 0 I user process page table entry; entry_point = 0v1000_8EF0; frame = Frame(32004 @ 0p7D0_4000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:40:13.663 0 D process_frames = 155
20:40:13.667 0 I switch to; address_space = "1:1" @ 0p7D8_6000
20:40:13.671 0 D switched to address_space
20:40:13.675 0 D set pid
20:40:13.677 0 D set current_process
20:40:13.681 0 D entering the user mode; pid = 1:1; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1000_8EF0, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags:  } }
20:40:13.695 0 I syscall = "sched_yield"; pid = 1:1
20:40:13.699 0 D leaving the user mode; pid = 1:1
20:40:13.703 0 D returned from the user space; user_registers = [0, 0, 0, 0, 140187732365312, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
20:40:13.711 0 I switch to; address_space = "1:1" @ 0p7D8_6000
20:40:13.715 0 D switched to address_space
20:40:13.719 0 D set pid
20:40:13.723 0 D set current_process
20:40:13.727 0 D entering the user mode; pid = 1:1; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1000_841E, ss:rsp: 0x001B:0v7F7F_FFFF_EEB8, rflags:  } }
20:40:13.739 0 I syscall = "sched_yield"; pid = 1:1
20:40:13.743 0 D leaving the user mode; pid = 1:1
20:40:13.747 0 D returned from the user space; user_registers = [0, 0, 0, 0, 140187732365312, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
20:40:13.755 0 I switch to; address_space = "1:1" @ 0p7D8_6000
20:40:13.759 0 D switched to address_space
20:40:13.763 0 D set pid
20:40:13.767 0 D set current_process
20:40:13.771 0 D entering the user mode; pid = 1:1; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1000_841E, ss:rsp: 0x001B:0v7F7F_FFFF_EEB8, rflags:  } }
20:40:13.785 0 I syscall = "sched_yield"; pid = 1:1
20:40:13.789 0 D leaving the user mode; pid = 1:1
20:40:13.793 0 I free; slot = Process { pid: 1:1, address_space: "1:1" @ 0p7D8_6000, { rip: 0v1000_841E, rsp: 0v7F7F_FFFF_EEB8 } }; process_count = 0
20:40:13.805 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 9, unlocks: 9, waits: 0 }
20:40:13.815 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:40:13.823 0 I switch to; address_space = "base" @ 0p1000
20:40:13.827 0 I drop the current address space; address_space = "1:1" @ 0p7D8_6000; switch_to = "base" @ 0p1000
4_process_7_scheduler::syscall_sched_yield--------- [passed]
20:40:14.623 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/process/scheduler.rs | 26 +++++++++++++++++++++-----
 kernel/src/process/syscall.rs   | 16 +++++++++++++---
 2 files changed, 34 insertions(+), 8 deletions(-)
```
