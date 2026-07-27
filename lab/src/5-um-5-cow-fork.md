## Copy-on-write fork

Теперь у нас есть всё необходимое для реализации copy-on-write `fork()` в пространстве пользователя.
Программа
[`user/cow_fork/src/main.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/cow_fork/src/main.rs)
структурно похожа на
[`user/eager_fork/src/main.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/eager_fork/src/main.rs)
и на `eager_fork` можно ориентироваться при реализации.


#### Ленивое копирование адресного пространства

```rust
fn copy_page_table(
    child: Pid,
    level: u32,
    trap_stack: Block<Page>,
    virt: Virt,
) -> Result<()>
```

Выполняется аналогично [соответствующей процедуре](../../lab/book/5-um-3-eager-fork.html#copy_page_table) `eager_fork`.
Отличается от которой в паре моментов:

- К игнорируемым страницам добавляется `trap_stack`, его копировать не нужно. У потомка изначально будет полностью отдельный стек для обработки исключений.
- Вместо копирования страниц функцией [`lib::memory::copy_page()`](../../doc/lib/memory/fn.copy_page.html) создаёт в потомке отображение. Оно ссылается на тот же физический фрейм, на который ссылается соответствующая виртуальная страница в родителе. При этом для страниц, которые отображены с одним из флагов [`PageTableFlags::WRITABLE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.WRITABLE) или [`PageTableFlags::COPY_ON_WRITE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.COPY_ON_WRITE), в обоих адресных пространствах меняет флаги отображения страницы так, чтобы [`PageTableFlags::COPY_ON_WRITE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.COPY_ON_WRITE) был включён, а [`PageTableFlags::WRITABLE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.WRITABLE) --- выключен. Копирование содержимого страницы функцией [`lib::memory::copy_page()`](../../doc/lib/memory/fn.copy_page.html) таким образом лениво откладывается до возникновения Page Fault.


#### Пользовательский обработчик исключений Page Fault

Когда программа попытается записать в страницу, помеченную в `copy_page_table()` как
[`PageTableFlags::COPY_ON_WRITE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.COPY_ON_WRITE)
и только на чтение,
возникнет Page Fault и ядро передаст управление в
[реализованный вами ранее](../../lab/book/5-um-4-trap-handler.html#%D0%A2%D1%80%D0%B0%D0%BC%D0%BF%D0%BB%D0%B8%D0%BD-%D0%BE%D0%B1%D1%80%D0%B0%D0%B1%D0%BE%D1%82%D1%87%D0%B8%D0%BA%D0%B0-%D0%BF%D1%80%D0%B5%D1%80%D1%8B%D0%B2%D0%B0%D0%BD%D0%B8%D0%B9)
[`lib::syscall::trap_trampoline()`](../../doc/lib/syscall/fn.trap_trampoline.html),
который в свою очередь запустит

```rust
fn trap_handler(info: &TrapInfo)
```

Эта функция работает в очень стеснённых условиях.

Возможно, вся память программы, кроме стека `trap_stack`, на котором сейчас работает эта функция,
доступна только на чтение.
В том числе
[`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html),
который используется для логирования в пространстве пользователя
макросами библиотеки [tracing](https://docs.rs/tracing/) --- `info!()`, `debug!()` и т.д.
Так как при этом нужно писать, то такое логирование в `trap_handler()` не доступно.
Макрос
[`panic!()`](https://doc.rust-lang.org/nightly/core/macro.panic.html)
тоже не доступен, так как он использует логирование.
В других частях `cow_fork` логированием можно пользоваться,
потому что `trap_handler()` починит возникающие при этом Page Fault.
Для логирования в `trap_handler()` можно воспользоваться
[одним из первых реализованных системных вызовов](../../lab/book/4-process-4-syscall.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-12--%D1%81%D0%B8%D1%81%D1%82%D0%B5%D0%BC%D0%BD%D1%8B%D0%B9-%D0%B2%D1%8B%D0%B7%D0%BE%D0%B2-log_value) ---
[`lib::syscall::log_value()`](../../doc/lib/syscall/fn.log_value.html).

Вторая неприятность заключается в том, что в процессе--потомке в `trap_handler()` не доступны
[`ku::ProcessInfo`](../../doc/ku/info/struct.ProcessInfo.html) и
[`ku::SystemInfo`](../../doc/ku/info/struct.SystemInfo.html) даже на чтение.
В результате, потомок не может узнать свой идентификатор
[`Pid`](../../doc/ku/process/pid/enum.Pid.html).
Поэтому в `trap_handler()` идентифицировать процесс для выполняемых системных вызовов нужно как
[`Pid::Current`](../../doc/ku/process/pid/enum.Pid.html#variant.Current).

Также учтите, что эти ограничения распространяются на вспомогательные функции, которые `trap_handler()` использует.
Впрочем, оба эти ограничения --- во многом следствие нашей реализации `cow_fork`,
а не характерная особенность обработки прерываний в пространстве пользователя.

При получении управления, `trap_handler()`:

- Проверяет, что прерывание --- это действительно `PageFault` и он вызван записью. Иначе обработчик прекращает исполнение программы, вызвав [`lib::syscall::exit()`](../../doc/lib/syscall/fn.exit.html) с кодом ошибки.
- С помощью [реализованной ранее](../../lab/book/5-um-3-eager-fork.html#temp_page) функции [`lib::memory::temp_page()`](../../doc/lib/memory/fn.temp_page.html) находит временную страницу.
- Копирует содержимое страницы, обращение к которой привело к Page Fault, во временную, с помощью [реализованной вами ранее](../../lab/book/5-um-3-eager-fork.html#copy_page) функции [`lib::memory::copy_page()`](../../doc/lib/memory/fn.copy_page.html).
- С помощью системного вызова [`lib::syscall::copy_mapping()`](../../doc/lib/syscall/fn.copy_mapping.html) заменяет физический фрейм под скопированной страницей на фрейм временной страницы, одновременно меняя флаг [`PageTableFlags::COPY_ON_WRITE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.COPY_ON_WRITE) на [`PageTableFlags::WRITABLE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.WRITABLE) в её отображении.
- С помощью системного вызова [`lib::syscall::unmap()`](../../doc/lib/syscall/fn.unmap.html) удаляет из адресного пространства не нужную более временную страницу.


#### `cow_fork()`

```rust
fn cow_fork(
    trap_stack: Block<Page>,
) -> Result<bool>
```

Эта функции похожа на соответствующую функцию `eager_fork()`, но в ней добавляется работа по инициализации обработчика прерываний.
Функция `cow_fork()`:

- Выделяет себе --- родительскому процессу --- стек для обработки исключений с помощью [`lib::syscall::map()`](../../doc/lib/syscall/fn.map.html) и устанавливает себе функцией [`lib::syscall::set_trap_handler()`](../../doc/lib/syscall/fn.set_trap_handler.html) обработчик исключений `trap_handler()`.
- Создаёт процесс потомка с помощью [реализованного вами ранее](../../lab/book/5-um-3-eager-fork.html#%D0%A1%D0%B8%D1%81%D1%82%D0%B5%D0%BC%D0%BD%D1%8B%D0%B9-%D0%B2%D1%8B%D0%B7%D0%BE%D0%B2-exofork) системного вызова [`lib::syscall::exofork()`](../../doc/lib/syscall/fn.exofork.html).
- Далее лениво копирует своё адресное пространство в пространство потомка с помощью функции `fn copy_address_space()`.
- Выделяет потомку отдельный стек для обработки исключений с помощью [`lib::syscall::map()`](../../doc/lib/syscall/fn.map.html) и устанавливает уже ему обработчик исключений `trap_handler()` функцией [`lib::syscall::set_trap_handler()`](../../doc/lib/syscall/fn.set_trap_handler.html).
- Запускает потомка системным вызовом [`lib::syscall::set_state()`](../../doc/lib/syscall/fn.set_state.html), устанавливая его состояние в [`State::Runnable`](../../doc/ku/process/enum.State.html#variant.Runnable).

В потомке `cow_fork()` ничего не делает.
Возвращает она `true` в процессе потомка и `false` в процессе родителя.


### Проверьте себя

Теперь должны заработать тест `cow_fork()` в файле
[`kernel/tests/5-um-5-cow-fork.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/5-um-5-cow-fork.rs):

```console
$ (cd kernel; cargo test --test 5-um-5-cow-fork)
...
5_um_5_cow_fork::cow_fork-----------------------------------
20:55:20 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:55:20 0 I duplicate; address_space = "process" @ 0p7E2_5000
20:55:20 0 I switch to; address_space = "process" @ 0p7E2_5000
20:55:20 0 D extend mapping; block = [0v1000_0000, 0v1000_9EBC), size 39.684 KiB; page_block = [0v1000_0000, 0v1000_A000), size 40.000 KiB
20:55:20 0 D elf loadable program header; file_block = [0v20_3000, 0v20_CEBC), size 39.684 KiB; memory_block = [0v1000_0000, 0v1000_9EBC), size 39.684 KiB
20:55:20 0 D extend mapping; block = [0v1000_A000, 0v1006_1FCF), size 351.952 KiB; page_block = [0v1000_A000, 0v1006_2000), size 352.000 KiB
20:55:20 0 D elf loadable program header; file_block = [0v20_CEC0, 0v26_4FCF), size 352.265 KiB; memory_block = [0v1000_9EC0, 0v1006_1FCF), size 352.265 KiB
20:55:20 0 D extend mapping; block = [0v1006_2000, 0v1006_20D8), size 216 B; page_block = [0v1006_2000, 0v1006_3000), size 4.000 KiB
20:55:20 0 D elf loadable program header; file_block = [0v26_4FD0, 0v26_50D8), size 264 B; memory_block = [0v1006_1FD0, 0v1006_20D8), size 264 B
20:55:20 0 D extend mapping; block = [0v1006_3000, 0v1006_A800), size 30.000 KiB; page_block = [0v1006_3000, 0v1006_B000), size 32.000 KiB
20:55:20 0 D elf loadable program header; file_block = [0v26_50D8, 0v26_D7D0), size 33.742 KiB; memory_block = [0v1006_20D8, 0v1006_A800), size 33.789 KiB
20:55:20 0 I switch to; address_space = "base" @ 0p1000
20:55:20 0 I loaded ELF file; context = { rip: 0v1001_41B0, rsp: 0v7F7F_FFFF_F000 }; file_size = 7.040 MiB; process = { pid: <current>, address_space: "process" @ 0p7E2_5000, { rip: 0v1001_41B0, rsp: 0v7F7F_FFFF_F000 } }
20:55:20 0 I allocate; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_5000, { rip: 0v1001_41B0, rsp: 0v7F7F_FFFF_F000 } }; process_count = 1
20:55:20 0 I user process page table entry; entry_point = 0v1001_41B0; frame = Frame(32250 @ 0p7DF_A000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:55:20 0 D process_frames = 169
20:55:20 0 I dequeue; pid = Some(0:0)
20:55:20 0 I switch to; address_space = "0:0" @ 0p7E2_5000
20:55:20 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1001_41B0, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags: IF } }
20:55:20 0 I name = "cow_fork *"; pedigree = [0:0]; len = 1; capacity = 3; pid = 0:0
20:55:21.193 0 I page allocator init; free_page_count = 33688649728; block = [0v180_0000_0000, 0v7F00_0000_0000), size 125.500 TiB
20:55:21.203 0 I duplicate; address_space = "process" @ 0p7D7_8000
20:55:21.207 0 I switch to; address_space = "process" @ 0p7D7_8000
20:55:21.213 0 I switch to; address_space = "0:0" @ 0p7E2_5000
20:55:21.219 0 I allocate; slot = Process { pid: 1:0, address_space: "1:0" @ 0p7D7_8000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D728 } }; process_count = 2
20:55:21.227 0 I syscall = "exofork"; process = 0:0; child = 1:0
20:55:21.233 0 I syscall::exofork() done; child = Ok(1:0); pid = 0:0
20:55:21.353 0 D leaving the user mode; pid = 0:0
20:55:21.357 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1001_D4B2, ss:rsp: 0x001B:0v7F7F_FFFF_C360, rflags: IF SF PF CF }
20:55:21.371 0 I dequeue; pid = Some(0:0)
20:55:21.375 0 I switch to; address_space = "0:0" @ 0p7E2_5000
20:55:21.387 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1001A000, rdi: 0x7F7FFFFFC448, rsi: 0x7F7FFFFFC458, { mode: user, cs:rip: 0x0023:0v1001_D4B2, ss:rsp: 0x001B:0v7F7F_FFFF_C360, rflags: IF SF PF CF } }
20:55:21.879 0 I copy_address_space done; child = 1:0; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 0:0
20:55:21.937 0 I syscall::set_state(); child = 1:0; result = Ok(()); pid = 0:0
20:55:22.273 0 I page allocator init; free_page_count = 33688649728; block = [0v180_0000_0000, 0v7F00_0000_0000), size 125.500 TiB
20:55:22.281 0 I duplicate; address_space = "process" @ 0p7D4_9000
20:55:22.285 0 I switch to; address_space = "process" @ 0p7D4_9000
20:55:22.291 0 I switch to; address_space = "0:0" @ 0p7E2_5000
20:55:22.295 0 I allocate; slot = Process { pid: 2:0, address_space: "2:0" @ 0p7D4_9000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D728 } }; process_count = 3
20:55:22.303 0 I syscall = "exofork"; process = 0:0; child = 2:0
20:55:22.307 0 I syscall::exofork() done; child = Ok(2:0); pid = 0:0
20:55:22.853 0 D leaving the user mode; pid = 0:0
20:55:22.855 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1001_2E50, ss:rsp: 0x001B:0v7F7F_FFFD_4FC8, rflags: IF }
20:55:22.867 0 I dequeue; pid = Some(1:0)
20:55:22.869 0 I switch to; address_space = "1:0" @ 0p7D7_8000
20:55:22.883 0 D entering the user mode; pid = 1:0; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7EFFFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D728, rflags: IF } }
20:55:22.909 0 I syscall::exofork() done; child = Ok(<current>); pid = 1:0
20:55:22.925 0 I just created; child = <current>; pid = 1:0; pid = 1:0
20:55:22.925 0 I name = "cow_fork *0"; pedigree = [0:0, 1:0]; len = 2; capacity = 3; pid = 1:0
20:55:23.283 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:23.291 0 I duplicate; address_space = "process" @ 0p7D8_3000
20:55:23.295 0 I switch to; address_space = "process" @ 0p7D8_3000
20:55:23.303 0 I switch to; address_space = "1:0" @ 0p7D7_8000
20:55:23.307 0 I allocate; slot = Process { pid: 3:0, address_space: "3:0" @ 0p7D8_3000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 4
20:55:23.315 0 I syscall = "exofork"; process = 1:0; child = 3:0
20:55:23.319 0 I syscall::exofork() done; child = Ok(3:0); pid = 1:0
20:55:23.751 0 D leaving the user mode; pid = 1:0
20:55:23.755 0 I the process was preempted; pid = 1:0; user_context = { mode: user, cs:rip: 0x0023:0v1001_6660, ss:rsp: 0x001B:0v7F7F_FFFF_C2A0, rflags: IF }
20:55:23.765 0 I dequeue; pid = Some(0:0)
20:55:23.769 0 I switch to; address_space = "0:0" @ 0p7E2_5000
20:55:23.781 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x0, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1001_2E50, ss:rsp: 0x001B:0v7F7F_FFFD_4FC8, rflags: IF } }
20:55:23.815 0 I copy_address_space done; child = 2:0; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 0:0
20:55:23.853 0 D leaving the user mode; pid = 0:0
20:55:23.855 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1001_2E50, ss:rsp: 0x001B:0v7F7F_FFFD_4FC8, rflags: IF AF }
20:55:23.867 0 I dequeue; pid = Some(1:0)
20:55:23.869 0 I switch to; address_space = "1:0" @ 0p7D7_8000
20:55:23.883 0 D entering the user mode; pid = 1:0; registers = { rax: 0xF0, rdi: 0x10063B98, rsi: 0x12, { mode: user, cs:rip: 0x0023:0v1001_6660, ss:rsp: 0x001B:0v7F7F_FFFF_C2A0, rflags: IF } }
20:55:24.047 0 I copy_address_space done; child = 3:0; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 1:0
20:55:24.081 0 I syscall::set_state(); child = 3:0; result = Ok(()); pid = 1:0
20:55:24.429 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:24.437 0 I duplicate; address_space = "process" @ 0p7CE_9000
20:55:24.441 0 I switch to; address_space = "process" @ 0p7CE_9000
20:55:24.447 0 I switch to; address_space = "1:0" @ 0p7D7_8000
20:55:24.453 0 I allocate; slot = Process { pid: 4:0, address_space: "4:0" @ 0p7CE_9000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 5
20:55:24.459 0 I syscall = "exofork"; process = 1:0; child = 4:0
20:55:24.463 0 I syscall::exofork() done; child = Ok(4:0); pid = 1:0
20:55:25.039 0 I copy_address_space done; child = 4:0; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 1:0
20:55:25.079 0 I syscall::set_state(); child = 4:0; result = Ok(()); pid = 1:0
20:55:25.431 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:25.439 0 I duplicate; address_space = "process" @ 0p7CB_7000
20:55:25.441 0 I switch to; address_space = "process" @ 0p7CB_7000
20:55:25.449 0 I switch to; address_space = "1:0" @ 0p7D7_8000
20:55:25.453 0 I allocate; slot = Process { pid: 5:0, address_space: "5:0" @ 0p7CB_7000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 6
20:55:25.461 0 I syscall = "exofork"; process = 1:0; child = 5:0
20:55:25.465 0 I syscall::exofork() done; child = Ok(5:0); pid = 1:0
20:55:26.043 0 I copy_address_space done; child = 5:0; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 1:0
20:55:26.081 0 I syscall::set_state(); child = 5:0; result = Ok(()); pid = 1:0
20:55:26.089 0 I free; slot = Process { pid: 1:0, address_space: "1:0" @ 0p7D7_8000, { rip: 0v1001_6660, rsp: 0v7F7F_FFFF_C2A0 } }; process_count = 5
20:55:26.097 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 1525, unlocks: 1525, waits: 0 }
20:55:26.105 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 3168, unlocks: 3168, waits: 0 }
20:55:26.113 0 I switch to; address_space = "base" @ 0p1000
20:55:26.117 0 I drop the current address space; address_space = "1:0" @ 0p7D7_8000; switch_to = "base" @ 0p1000
20:55:26.537 0 I syscall = "exit"; pid = 1:0; code = 0; reason = Ok(Ok)
20:55:26.541 0 D leaving the user mode; pid = 1:0
20:55:26.545 0 I dequeue; pid = Some(0:0)
20:55:26.549 0 I switch to; address_space = "0:0" @ 0p7E2_5000
20:55:26.561 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1006A7D0, rdi: 0x1006A7D0, rsi: 0x1000FC10, { mode: user, cs:rip: 0x0023:0v1001_2E50, ss:rsp: 0x001B:0v7F7F_FFFD_4FC8, rflags: IF AF } }
20:55:26.579 0 I syscall::set_state(); child = 2:0; result = Ok(()); pid = 0:0
20:55:26.921 0 I page allocator init; free_page_count = 33688649728; block = [0v180_0000_0000, 0v7F00_0000_0000), size 125.500 TiB
20:55:26.929 0 I duplicate; address_space = "process" @ 0p7D6_4000
20:55:26.933 0 I switch to; address_space = "process" @ 0p7D6_4000
20:55:26.939 0 I switch to; address_space = "0:0" @ 0p7E2_5000
20:55:26.943 0 I allocate; slot = Process { pid: 1:1, address_space: "1:1" @ 0p7D6_4000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D728 } }; process_count = 6
20:55:26.951 0 I syscall = "exofork"; process = 0:0; child = 1:1
20:55:26.955 0 I syscall::exofork() done; child = Ok(1:1); pid = 0:0
20:55:27.523 0 I copy_address_space done; child = 1:1; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 0:0
20:55:27.565 0 I syscall::set_state(); child = 1:1; result = Ok(()); pid = 0:0
20:55:27.571 0 I free; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_5000, { rip: 0v1001_2E50, rsp: 0v7F7F_FFFD_4FC8 } }; process_count = 5
20:55:27.579 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 1367, unlocks: 1367, waits: 0 }
20:55:27.587 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 3007, unlocks: 3007, waits: 0 }
20:55:27.593 0 I switch to; address_space = "base" @ 0p1000
20:55:27.597 0 I drop the current address space; address_space = "0:0" @ 0p7E2_5000; switch_to = "base" @ 0p1000
20:55:28.005 0 I syscall = "exit"; pid = 0:0; code = 0; reason = Ok(Ok)
20:55:28.009 0 D leaving the user mode; pid = 0:0
20:55:28.013 0 I dequeue; pid = Some(3:0)
20:55:28.017 0 I switch to; address_space = "3:0" @ 0p7D8_3000
20:55:28.029 0 D entering the user mode; pid = 3:0; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D238, rflags: IF } }
20:55:28.055 0 I syscall::exofork() done; child = Ok(<current>); pid = 3:0
20:55:28.071 0 I just created; child = <current>; pid = 3:0; pid = 3:0
20:55:28.071 0 I name = "cow_fork *00"; pedigree = [0:0, 1:0, 3:0]; len = 3; capacity = 3; pid = 3:0
20:55:28.089 0 I free; slot = Process { pid: 3:0, address_space: "3:0" @ 0p7D8_3000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 4
20:55:28.097 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 169, unlocks: 169, waits: 0 }
20:55:28.103 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 164, unlocks: 164, waits: 0 }
20:55:28.111 0 I switch to; address_space = "base" @ 0p1000
20:55:28.115 0 I drop the current address space; address_space = "3:0" @ 0p7D8_3000; switch_to = "base" @ 0p1000
20:55:28.543 0 I syscall = "exit"; pid = 3:0; code = 0; reason = Ok(Ok)
20:55:28.547 0 D leaving the user mode; pid = 3:0
20:55:28.551 0 I dequeue; pid = Some(4:0)
20:55:28.553 0 I switch to; address_space = "4:0" @ 0p7CE_9000
20:55:28.567 0 D entering the user mode; pid = 4:0; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D238, rflags: IF } }
20:55:28.593 0 I syscall::exofork() done; child = Ok(<current>); pid = 4:0
20:55:28.609 0 I just created; child = <current>; pid = 4:0; pid = 4:0
20:55:28.609 0 I name = "cow_fork *01"; pedigree = [0:0, 1:0, 4:0]; len = 3; capacity = 3; pid = 4:0
20:55:28.627 0 I free; slot = Process { pid: 4:0, address_space: "4:0" @ 0p7CE_9000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 3
20:55:28.635 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 169, unlocks: 169, waits: 0 }
20:55:28.641 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 164, unlocks: 164, waits: 0 }
20:55:28.647 0 I switch to; address_space = "base" @ 0p1000
20:55:28.651 0 I drop the current address space; address_space = "4:0" @ 0p7CE_9000; switch_to = "base" @ 0p1000
20:55:29.079 0 I syscall = "exit"; pid = 4:0; code = 0; reason = Ok(Ok)
20:55:29.083 0 D leaving the user mode; pid = 4:0
20:55:29.087 0 I dequeue; pid = Some(5:0)
20:55:29.089 0 I switch to; address_space = "5:0" @ 0p7CB_7000
20:55:29.103 0 D entering the user mode; pid = 5:0; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D238, rflags: IF } }
20:55:29.129 0 I syscall::exofork() done; child = Ok(<current>); pid = 5:0
20:55:29.143 0 I just created; child = <current>; pid = 5:0; pid = 5:0
20:55:29.145 0 I name = "cow_fork *02"; pedigree = [0:0, 1:0, 5:0]; len = 3; capacity = 3; pid = 5:0
20:55:29.161 0 I free; slot = Process { pid: 5:0, address_space: "5:0" @ 0p7CB_7000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 2
20:55:29.169 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 169, unlocks: 169, waits: 0 }
20:55:29.177 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 164, unlocks: 164, waits: 0 }
20:55:29.183 0 I switch to; address_space = "base" @ 0p1000
20:55:29.187 0 I drop the current address space; address_space = "5:0" @ 0p7CB_7000; switch_to = "base" @ 0p1000
20:55:29.619 0 I syscall = "exit"; pid = 5:0; code = 0; reason = Ok(Ok)
20:55:29.623 0 D leaving the user mode; pid = 5:0
20:55:29.627 0 I dequeue; pid = Some(2:0)
20:55:29.631 0 I switch to; address_space = "2:0" @ 0p7D4_9000
20:55:29.643 0 D entering the user mode; pid = 2:0; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7EFFFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D728, rflags: IF SF PF CF } }
20:55:29.669 0 I syscall::exofork() done; child = Ok(<current>); pid = 2:0
20:55:29.685 0 I just created; child = <current>; pid = 2:0; pid = 2:0
20:55:29.685 0 I name = "cow_fork *1"; pedigree = [0:0, 2:0]; len = 2; capacity = 3; pid = 2:0
20:55:30.043 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:30.051 0 I duplicate; address_space = "process" @ 0p7D5_6000
20:55:30.055 0 I switch to; address_space = "process" @ 0p7D5_6000
20:55:30.063 0 I switch to; address_space = "2:0" @ 0p7D4_9000
20:55:30.067 0 I allocate; slot = Process { pid: 5:1, address_space: "5:1" @ 0p7D5_6000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 3
20:55:30.075 0 I syscall = "exofork"; process = 2:0; child = 5:1
20:55:30.079 0 I syscall::exofork() done; child = Ok(5:1); pid = 2:0
20:55:30.651 0 I copy_address_space done; child = 5:1; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 2:0
20:55:30.691 0 I syscall::set_state(); child = 5:1; result = Ok(()); pid = 2:0
20:55:31.037 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:31.045 0 I duplicate; address_space = "process" @ 0p7CB_3000
20:55:31.049 0 I switch to; address_space = "process" @ 0p7CB_3000
20:55:31.057 0 I switch to; address_space = "2:0" @ 0p7D4_9000
20:55:31.061 0 I allocate; slot = Process { pid: 4:1, address_space: "4:1" @ 0p7CB_3000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 4
20:55:31.069 0 I syscall = "exofork"; process = 2:0; child = 4:1
20:55:31.073 0 I syscall::exofork() done; child = Ok(4:1); pid = 2:0
20:55:31.651 0 I copy_address_space done; child = 4:1; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 2:0
20:55:31.689 0 I syscall::set_state(); child = 4:1; result = Ok(()); pid = 2:0
20:55:32.041 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:32.049 0 I duplicate; address_space = "process" @ 0p7CE_6000
20:55:32.053 0 I switch to; address_space = "process" @ 0p7CE_6000
20:55:32.059 0 I switch to; address_space = "2:0" @ 0p7D4_9000
20:55:32.065 0 I allocate; slot = Process { pid: 3:1, address_space: "3:1" @ 0p7CE_6000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 5
20:55:32.073 0 I syscall = "exofork"; process = 2:0; child = 3:1
20:55:32.075 0 I syscall::exofork() done; child = Ok(3:1); pid = 2:0
20:55:32.651 0 I copy_address_space done; child = 3:1; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 2:0
20:55:32.689 0 I syscall::set_state(); child = 3:1; result = Ok(()); pid = 2:0
20:55:32.697 0 I free; slot = Process { pid: 2:0, address_space: "2:0" @ 0p7D4_9000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D728 } }; process_count = 4
20:55:32.705 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 1519, unlocks: 1519, waits: 0 }
20:55:32.711 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 3164, unlocks: 3164, waits: 0 }
20:55:32.717 0 I switch to; address_space = "base" @ 0p1000
20:55:32.721 0 I drop the current address space; address_space = "2:0" @ 0p7D4_9000; switch_to = "base" @ 0p1000
20:55:33.143 0 I syscall = "exit"; pid = 2:0; code = 0; reason = Ok(Ok)
20:55:33.147 0 D leaving the user mode; pid = 2:0
20:55:33.151 0 I dequeue; pid = Some(1:1)
20:55:33.153 0 I switch to; address_space = "1:1" @ 0p7D6_4000
20:55:33.167 0 D entering the user mode; pid = 1:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7EFFFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D728, rflags: IF AF } }
20:55:33.193 0 I syscall::exofork() done; child = Ok(<current>); pid = 1:1
20:55:33.207 0 I just created; child = <current>; pid = 1:1; pid = 1:1
20:55:33.209 0 I name = "cow_fork *2"; pedigree = [0:0, 1:1]; len = 2; capacity = 3; pid = 1:1
20:55:33.571 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:33.579 0 I duplicate; address_space = "process" @ 0p7D0_1000
20:55:33.583 0 I switch to; address_space = "process" @ 0p7D0_1000
20:55:33.589 0 I switch to; address_space = "1:1" @ 0p7D6_4000
20:55:33.593 0 I allocate; slot = Process { pid: 2:1, address_space: "2:1" @ 0p7D0_1000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 5
20:55:33.601 0 I syscall = "exofork"; process = 1:1; child = 2:1
20:55:33.605 0 I syscall::exofork() done; child = Ok(2:1); pid = 1:1
20:55:34.179 0 I copy_address_space done; child = 2:1; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 1:1
20:55:34.219 0 I syscall::set_state(); child = 2:1; result = Ok(()); pid = 1:1
20:55:34.571 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:34.579 0 I duplicate; address_space = "process" @ 0p7E1_0000
20:55:34.583 0 I switch to; address_space = "process" @ 0p7E1_0000
20:55:34.589 0 I switch to; address_space = "1:1" @ 0p7D6_4000
20:55:34.593 0 I allocate; slot = Process { pid: 0:1, address_space: "0:1" @ 0p7E1_0000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 6
20:55:34.601 0 I syscall = "exofork"; process = 1:1; child = 0:1
20:55:34.605 0 I syscall::exofork() done; child = Ok(0:1); pid = 1:1
20:55:35.151 0 D leaving the user mode; pid = 1:1
20:55:35.153 0 I the process was preempted; pid = 1:1; user_context = { mode: user, cs:rip: 0x0023:0v1001_2E50, ss:rsp: 0x001B:0v7F7F_FFFD_4FC8, rflags: IF }
20:55:35.165 0 I dequeue; pid = Some(5:1)
20:55:35.167 0 I switch to; address_space = "5:1" @ 0p7D5_6000
20:55:35.181 0 D entering the user mode; pid = 5:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D238, rflags: IF SF PF CF } }
20:55:35.207 0 I syscall::exofork() done; child = Ok(<current>); pid = 5:1
20:55:35.223 0 I just created; child = <current>; pid = 5:1; pid = 5:1
20:55:35.223 0 I name = "cow_fork *10"; pedigree = [0:0, 2:0, 5:1]; len = 3; capacity = 3; pid = 5:1
20:55:35.241 0 I free; slot = Process { pid: 5:1, address_space: "5:1" @ 0p7D5_6000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 5
20:55:35.247 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 169, unlocks: 169, waits: 0 }
20:55:35.255 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 164, unlocks: 164, waits: 0 }
20:55:35.261 0 I switch to; address_space = "base" @ 0p1000
20:55:35.265 0 I drop the current address space; address_space = "5:1" @ 0p7D5_6000; switch_to = "base" @ 0p1000
20:55:35.693 0 I syscall = "exit"; pid = 5:1; code = 0; reason = Ok(Ok)
20:55:35.697 0 D leaving the user mode; pid = 5:1
20:55:35.701 0 I dequeue; pid = Some(4:1)
20:55:35.705 0 I switch to; address_space = "4:1" @ 0p7CB_3000
20:55:35.717 0 D entering the user mode; pid = 4:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D238, rflags: IF SF PF CF } }
20:55:35.743 0 I syscall::exofork() done; child = Ok(<current>); pid = 4:1
20:55:35.759 0 I just created; child = <current>; pid = 4:1; pid = 4:1
20:55:35.759 0 I name = "cow_fork *11"; pedigree = [0:0, 2:0, 4:1]; len = 3; capacity = 3; pid = 4:1
20:55:35.777 0 I free; slot = Process { pid: 4:1, address_space: "4:1" @ 0p7CB_3000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 4
20:55:35.783 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 169, unlocks: 169, waits: 0 }
20:55:35.791 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 164, unlocks: 164, waits: 0 }
20:55:35.797 0 I switch to; address_space = "base" @ 0p1000
20:55:35.801 0 I drop the current address space; address_space = "4:1" @ 0p7CB_3000; switch_to = "base" @ 0p1000
20:55:36.229 0 I syscall = "exit"; pid = 4:1; code = 0; reason = Ok(Ok)
20:55:36.233 0 D leaving the user mode; pid = 4:1
20:55:36.237 0 I dequeue; pid = Some(3:1)
20:55:36.239 0 I switch to; address_space = "3:1" @ 0p7CE_6000
20:55:36.253 0 D entering the user mode; pid = 3:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D238, rflags: IF SF PF CF } }
20:55:36.279 0 I syscall::exofork() done; child = Ok(<current>); pid = 3:1
20:55:36.293 0 I just created; child = <current>; pid = 3:1; pid = 3:1
20:55:36.295 0 I name = "cow_fork *12"; pedigree = [0:0, 2:0, 3:1]; len = 3; capacity = 3; pid = 3:1
20:55:36.311 0 I free; slot = Process { pid: 3:1, address_space: "3:1" @ 0p7CE_6000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 3
20:55:36.319 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 169, unlocks: 169, waits: 0 }
20:55:36.327 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 164, unlocks: 164, waits: 0 }
20:55:36.333 0 I switch to; address_space = "base" @ 0p1000
20:55:36.337 0 I drop the current address space; address_space = "3:1" @ 0p7CE_6000; switch_to = "base" @ 0p1000
20:55:36.767 0 I syscall = "exit"; pid = 3:1; code = 0; reason = Ok(Ok)
20:55:36.771 0 D leaving the user mode; pid = 3:1
20:55:36.775 0 I dequeue; pid = Some(2:1)
20:55:36.779 0 I switch to; address_space = "2:1" @ 0p7D0_1000
20:55:36.791 0 D entering the user mode; pid = 2:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D238, rflags: IF AF } }
20:55:36.817 0 I syscall::exofork() done; child = Ok(<current>); pid = 2:1
20:55:36.833 0 I just created; child = <current>; pid = 2:1; pid = 2:1
20:55:36.833 0 I name = "cow_fork *20"; pedigree = [0:0, 1:1, 2:1]; len = 3; capacity = 3; pid = 2:1
20:55:36.851 0 I free; slot = Process { pid: 2:1, address_space: "2:1" @ 0p7D0_1000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 2
20:55:36.859 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 169, unlocks: 169, waits: 0 }
20:55:36.865 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 164, unlocks: 164, waits: 0 }
20:55:36.873 0 I switch to; address_space = "base" @ 0p1000
20:55:36.877 0 I drop the current address space; address_space = "2:1" @ 0p7D0_1000; switch_to = "base" @ 0p1000
20:55:37.307 0 I syscall = "exit"; pid = 2:1; code = 0; reason = Ok(Ok)
20:55:37.311 0 D leaving the user mode; pid = 2:1
20:55:37.315 0 I dequeue; pid = Some(1:1)
20:55:37.319 0 I switch to; address_space = "1:1" @ 0p7D6_4000
20:55:37.331 0 D entering the user mode; pid = 1:1; registers = { rax: 0x0, rdi: 0x0, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1001_2E50, ss:rsp: 0x001B:0v7F7F_FFFD_4FC8, rflags: IF } }
20:55:37.373 0 I copy_address_space done; child = 0:1; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 1:1
20:55:37.411 0 I syscall::set_state(); child = 0:1; result = Ok(()); pid = 1:1
20:55:37.767 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:37.775 0 I duplicate; address_space = "process" @ 0p7CF_8000
20:55:37.779 0 I switch to; address_space = "process" @ 0p7CF_8000
20:55:37.785 0 I switch to; address_space = "1:1" @ 0p7D6_4000
20:55:37.789 0 I allocate; slot = Process { pid: 2:2, address_space: "2:2" @ 0p7CF_8000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 3
20:55:37.797 0 I syscall = "exofork"; process = 1:1; child = 2:2
20:55:37.801 0 I syscall::exofork() done; child = Ok(2:2); pid = 1:1
20:55:38.377 0 I copy_address_space done; child = 2:2; trap_stack = [0v7F7F_FFFD_1000, 0v7F7F_FFFD_5000), size 16.000 KiB, Page[~127.500 TiB, ~127.500 TiB); pid = 1:1
20:55:38.415 0 I syscall::set_state(); child = 2:2; result = Ok(()); pid = 1:1
20:55:38.421 0 I free; slot = Process { pid: 1:1, address_space: "1:1" @ 0p7D6_4000, { rip: 0v1001_2E50, rsp: 0v7F7F_FFFD_4FC8 } }; process_count = 2
20:55:38.429 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 1521, unlocks: 1521, waits: 0 }
20:55:38.437 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 3164, unlocks: 3164, waits: 0 }
20:55:38.443 0 I switch to; address_space = "base" @ 0p1000
20:55:38.447 0 I drop the current address space; address_space = "1:1" @ 0p7D6_4000; switch_to = "base" @ 0p1000
20:55:38.871 0 I syscall = "exit"; pid = 1:1; code = 0; reason = Ok(Ok)
20:55:38.875 0 D leaving the user mode; pid = 1:1
20:55:38.879 0 I dequeue; pid = Some(0:1)
20:55:38.883 0 I switch to; address_space = "0:1" @ 0p7E1_0000
20:55:38.895 0 D entering the user mode; pid = 0:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D238, rflags: IF AF } }
20:55:38.921 0 I syscall::exofork() done; child = Ok(<current>); pid = 0:1
20:55:38.937 0 I just created; child = <current>; pid = 0:1; pid = 0:1
20:55:38.937 0 I name = "cow_fork *21"; pedigree = [0:0, 1:1, 0:1]; len = 3; capacity = 3; pid = 0:1
20:55:38.955 0 I free; slot = Process { pid: 0:1, address_space: "0:1" @ 0p7E1_0000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 1
20:55:38.963 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 169, unlocks: 169, waits: 0 }
20:55:38.971 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 164, unlocks: 164, waits: 0 }
20:55:38.977 0 I switch to; address_space = "base" @ 0p1000
20:55:38.981 0 I drop the current address space; address_space = "0:1" @ 0p7E1_0000; switch_to = "base" @ 0p1000
20:55:39.411 0 I syscall = "exit"; pid = 0:1; code = 0; reason = Ok(Ok)
20:55:39.415 0 D leaving the user mode; pid = 0:1
20:55:39.419 0 I dequeue; pid = Some(2:2)
20:55:39.423 0 I switch to; address_space = "2:2" @ 0p7CF_8000
20:55:39.435 0 D entering the user mode; pid = 2:2; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_D513, ss:rsp: 0x001B:0v7F7F_FFFF_D238, rflags: IF } }
20:55:39.461 0 I syscall::exofork() done; child = Ok(<current>); pid = 2:2
20:55:39.477 0 I just created; child = <current>; pid = 2:2; pid = 2:2
20:55:39.477 0 I name = "cow_fork *22"; pedigree = [0:0, 1:1, 2:2]; len = 3; capacity = 3; pid = 2:2
20:55:39.495 0 I free; slot = Process { pid: 2:2, address_space: "2:2" @ 0p7CF_8000, { rip: 0v1000_D513, rsp: 0v7F7F_FFFF_D238 } }; process_count = 0
20:55:39.503 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 169, unlocks: 169, waits: 0 }
20:55:39.509 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 164, unlocks: 164, waits: 0 }
20:55:39.517 0 I switch to; address_space = "base" @ 0p1000
20:55:39.521 0 I drop the current address space; address_space = "2:2" @ 0p7CF_8000; switch_to = "base" @ 0p1000
20:55:39.955 0 I syscall = "exit"; pid = 2:2; code = 0; reason = Ok(Ok)
20:55:39.959 0 D leaving the user mode; pid = 2:2
20:55:39.963 0 I dequeue; pid = None
20:55:39.971 0 D parent = 0:0; process = 1:0
20:55:39.979 0 D parent = 0:0; process = 1:1
20:55:39.983 0 D parent = 0:0; process = 2:0
20:55:39.987 0 D parent = 1:0; process = 3:0
20:55:39.991 0 D parent = 1:0; process = 4:0
20:55:39.995 0 D parent = 1:0; process = 5:0
20:55:39.999 0 D parent = 1:1; process = 0:1
20:55:40.001 0 D parent = 1:1; process = 2:1
20:55:40.005 0 D parent = 1:1; process = 2:2
20:55:40.007 0 D parent = 2:0; process = 3:1
20:55:40.011 0 D parent = 2:0; process = 4:1
20:55:40.015 0 D parent = 2:0; process = 5:1
20:55:40.019 0 D graphviz = digraph process_tree { node [ style = filled; fillcolor = "#CCCCCC"]; "0:0" -> "1:0"; "0:0" -> "1:1"; "0:0" -> "2:0"; "1:0" -> "3:0"; "1:0" -> "4:0"; "1:0" -> "5:0"; "1:1" -> "0:1"; "1:1" -> "2:1"; "1:1" -> "2:2"; "2:0" -> "3:1"; "2:0" -> "4:1"; "2:0" -> "5:1"; }
5_um_5_cow_fork::cow_fork-------------------------- [passed]
20:55:40.037 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```

```console
$ grep 'pedigree' log
20:55:20 0 I name = "cow_fork *"; pedigree = [0:0]; len = 1; capacity = 3; pid = 0:0
20:55:22.925 0 I name = "cow_fork *0"; pedigree = [0:0, 1:0]; len = 2; capacity = 3; pid = 1:0
20:55:28.071 0 I name = "cow_fork *00"; pedigree = [0:0, 1:0, 3:0]; len = 3; capacity = 3; pid = 3:0
20:55:28.609 0 I name = "cow_fork *01"; pedigree = [0:0, 1:0, 4:0]; len = 3; capacity = 3; pid = 4:0
20:55:29.145 0 I name = "cow_fork *02"; pedigree = [0:0, 1:0, 5:0]; len = 3; capacity = 3; pid = 5:0
20:55:29.685 0 I name = "cow_fork *1"; pedigree = [0:0, 2:0]; len = 2; capacity = 3; pid = 2:0
20:55:33.209 0 I name = "cow_fork *2"; pedigree = [0:0, 1:1]; len = 2; capacity = 3; pid = 1:1
20:55:35.223 0 I name = "cow_fork *10"; pedigree = [0:0, 2:0, 5:1]; len = 3; capacity = 3; pid = 5:1
20:55:35.759 0 I name = "cow_fork *11"; pedigree = [0:0, 2:0, 4:1]; len = 3; capacity = 3; pid = 4:1
20:55:36.295 0 I name = "cow_fork *12"; pedigree = [0:0, 2:0, 3:1]; len = 3; capacity = 3; pid = 3:1
20:55:36.833 0 I name = "cow_fork *20"; pedigree = [0:0, 1:1, 2:1]; len = 3; capacity = 3; pid = 2:1
20:55:38.937 0 I name = "cow_fork *21"; pedigree = [0:0, 1:1, 0:1]; len = 3; capacity = 3; pid = 0:1
20:55:39.477 0 I name = "cow_fork *22"; pedigree = [0:0, 1:1, 2:2]; len = 3; capacity = 3; pid = 2:2
```

Получилось такое дерево процессов:
![](5-um-5-cow-fork.svg)


### Ориентировочный объём работ этой части лабораторки

```console
 user/cow_fork/src/main.rs | 111 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++-------
 1 file changed, 103 insertions(+), 8 deletions(-)
```
