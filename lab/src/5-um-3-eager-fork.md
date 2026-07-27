## Eager `fork()`

Системный вызов
[`fork()`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/fork.html)
долгое время считался удачной абстракцией.
Но время показало, что у него есть и большое количество недостатков,
см. статью
[A `fork()` in the road](https://www.microsoft.com/en-us/research/uploads/prod/2019/04/fork-hotos19.pdf).
Хотя не стоит воспринимать
[`fork()`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/fork.html)
как удачный интерфейс порождения процессов,
его реализация является хорошим упражнением.

Nikka воплощает некоторые идеи [экзоядра](https://en.wikipedia.org/wiki/Exokernel),
поэтому реализовывать
[`fork()`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/fork.html)
будем в пространстве пользователя.
Естественно, от ядра потребуется небольшая помощь.
Причём то, что реализует ядро, пригодится и для других целей.
Например, можно будет реализовать аналог современного системного вызова для порождения процессов ---
[`posix_spawn()`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/posix_spawn.html).
Естественно, также в пространстве пользователя.

Подробнее про концепцию экзоядра можно почитать в оригинальной статье ---
[Exokernel: an operating system architecture for application-level resource management](https://pdos.csail.mit.edu/6.828/2008/readings/engler95exokernel.pdf).


### Системный вызов [`kernel::process::syscall::exofork()`](../../doc/kernel/process/syscall/fn.exofork.html)

Реализуйте [системный вызов](../../doc/kernel/process/syscall/fn.exofork.html)

```rust
{{#include ../../kernel/src/process/syscall.rs:exofork}}
```

в файле [`kernel/src/process/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/syscall.rs).

Он создаёт копию вызывающего процесса `process` и возвращает его
[`Pid`](../../doc/ku/process/pid/enum.Pid.html).
При этом новый процесс создаётся практически без адресного пространства и не готовый к работе.
Поэтому он, в частности, не ставится в очередь планировщика.

Этот системный вызов использует создание копии системной части адресного пространства процесса `process`,
которое
[вы реализовали ранее](../../lab/book/2-mm-6-address-space-2-translate.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-6--%D1%81%D0%BE%D0%B7%D0%B4%D0%B0%D0%BD%D0%B8%D0%B5-%D0%BF%D0%BE%D0%BB%D0%BD%D0%BE%D0%B9-%D0%BA%D0%BE%D0%BF%D0%B8%D0%B8-%D0%B2%D0%B8%D1%80%D1%82%D1%83%D0%B0%D0%BB%D1%8C%D0%BD%D0%BE%D0%B3%D0%BE-%D0%BE%D1%82%D0%BE%D0%B1%D1%80%D0%B0%D0%B6%D0%B5%D0%BD%D0%B8%D1%8F).
А вот пользовательскую часть адресного пространства он не копирует.
Этим займётся код на стороне пользователя.

Системный вызов `exofork()` возвращает
[`Pid::Id`](../../doc/ku/process/pid/enum.Pid.html#variant.Id)
с идентификатором потомка в процессе родителя и константу
[`Pid::Current`](../../doc/ku/process/pid/enum.Pid.html#variant.Current)
в процессе потомка.

Текущий контекст родителя --- `context` --- нужно записать в потомка.
Вы уже делали аналогично для
[`sched_yield()`](../../doc/kernel/process/syscall/fn.sched_yield.html).
В родителя
[`exofork()`](../../doc/kernel/process/syscall/fn.exofork.html)
вернётся через функцию
[`kernel::process::syscall::sysret()`](../../doc/kernel/process/syscall/fn.sysret.html),
которая получает `context` на вход и восстанавливает его.
А вот потомок будет позже запущен планировщиком через цепочку
[`kernel::process::process::Process::enter_user_mode()`](../../doc/kernel/process/process/struct.Process.html#method.enter_user_mode) ->
[`kernel::process::registers::Registers::switch_to()`](../../doc/kernel/process/registers/struct.Registers.html#method.switch_to) ->
[`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq).
И для
[`iretq`](https://www.felixcloutier.com/x86/iret:iretd:iretq)
нужно предоставить правильный контекст пользователя, в который нужно будет переключиться.

Обратите внимание на код библиотечной обёртки
[`lib::syscall::exofork()`](../../doc/lib/syscall/fn.exofork.html)
для этого системного вызова
в файле [`user/lib/src/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/lib/src/syscall.rs).
Она предполагает, что ваша реализация функции
[`lib::syscall::syscall()`](../../doc/lib/syscall/fn.syscall.html)
возвращает `child_pid`:
```rust
let child_pid = syscall(Syscall::Exofork, 0, 0, 0, 0, 0)?;
```
В поле `child_pid` должен быть
[`Pid::Id`](../../doc/ku/process/pid/enum.Pid.html#variant.Id)
с идентификатором потомка в процессе родителя или
[`Pid::Current`](../../doc/ku/process/pid/enum.Pid.html#variant.Current)
в процессе потомка.


### Системный вызов [`kernel::process::syscall::set_state()`](../../doc/kernel/process/syscall/fn.set_state.html)

Реализуйте [системный вызов](../../doc/kernel/process/syscall/fn.set_state.html)

```rust
{{#include ../../kernel/src/process/syscall.rs:set_state}}
```

в файле [`kernel/src/process/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/syscall.rs).

Он переводит целевой процесс, заданный идентификатором `dst_pid`, в заданное состояние `state`.
И ставит его в очередь планировщика в случае
[`State::Runnable`](../../doc/ku/process/enum.State.html#variant.Runnable).
Не забудьте
проверить права доступа
процесса `process` к процессу `dst_pid` с помощью реализованного вами ранее
[`kernel::process::syscall::lock_set::LockSet`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html).


### Рекурсивное отображение памяти

Для того чтобы в пространстве пользователя процесс мог прочитать собственное
отображение страниц адресного пространства,
удобно использовать старый джедайский трюк ---
[рекурсивное отображение памяти](https://os.phil-opp.com/page-tables/#recursive-mapping).

Реализуйте [метод](../../doc/kernel/memory/mapping/struct.Mapping.html#method.make_recursive_mapping)

```rust
fn Mapping::make_recursive_mapping(&mut self) -> Result<usize>
```

в файле [`kernel/src/memory/mapping.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/memory/mapping.rs).
Выберете в таблице страниц корневого уровня свободную запись, например ближе к концу.
И используйте её как рекурсивную.
Сохраните её номер в поле
[`Mapping::recursive_mapping`](../../doc/kernel/memory/mapping/struct.Mapping.html#structfield.recursive_mapping)
и верните наружу.
Поправьте методы
[`Mapping::duplicate_page_table()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.duplicate_page_table) и
[`Mapping::drop_subtree()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.drop_subtree),
чтобы они игнорировали эту запись корневой таблицы страниц.


### Библиотечные функции

В файле
[`user/lib/src/memory/mod.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/lib/src/memory/mod.rs)
реализуйте вспомогательные функции кода пользователя.


#### [`lib::memory::temp_page()`](../../doc/lib/memory/fn.temp_page.html)

```rust
fn temp_page() -> Result<Page>
```

Заводит в адресном пространстве процесса страницу памяти для временных нужд.
Использует системный вызов
[`lib::syscall::map()`](../../doc/lib/syscall/fn.map.html),
определённый в файле
[`user/lib/src/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/lib/src/syscall.rs),
реализацию которого в ядре [вы уже написали](../../lab/book/5-um-2-memory.html#map).


#### [`lib::memory::copy_page()`](../../doc/lib/memory/fn.copy_page.html)

```rust
unsafe fn copy_page(src: Page, dst: Page)
```

Копирует содержимое страницы `src` в страницу `dst` с помощью
[core::ptr::copy_nonoverlapping()](https://doc.rust-lang.org/nightly/core/ptr/fn.copy_nonoverlapping.html).


#### [`lib::memory::page_table()`](../../doc/lib/memory/fn.page_table.html)

```rust
unsafe fn page_table(address: Virt, level: u32) -> &'static PageTable
```

Пользуясь рекурсивной записью таблицы страниц,
выдаёт ссылку на таблицу страниц заданного уровня `level` для заданного виртуального адреса `address`.
Узнать номер рекурсивной записи в пространстве пользователя можно методом
[`ProcessInfo::recursive_mapping()`](../../doc/ku/info/struct.ProcessInfo.html#method.recursive_mapping),
а получить саму структуру
[`ku::info::ProcessInfo`](../../doc/ku/info/struct.ProcessInfo.html)
можно функцией
[`ku::info::process_info()`](../../doc/ku/info/fn.process_info.html).

Вам может пригодиться метод
[`Virt::canonize()`](../../doc/ku/memory/addr/type.Virt.html#method.canonize).


### Основной код пользовательского процесса `eager_fork`

В файле [`user/eager_fork/src/main.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/eager_fork/src/main.rs) пользовательского процесса `eager_fork` реализуйте следующие функции.


#### `copy_page_table()`

```rust
{{#include ../../user/eager_fork/src/main.rs:copy_page_table}}
```

Копирует таблицу страниц уровня `level` для виртуального адреса `virt`
из своего адресного пространства в пространство дочернего процесса `child`.
Работает рекурсивно.
Корень рекурсии запускает функция

```rust
{{#include ../../user/eager_fork/src/main.rs:copy_address_space}}
```

Проходится по таблице страниц, получая ссылку на неё с помощью
[реализованной вами ранее](../../lab/book/5-um-3-eager-fork.html#page_table)
[`lib::memory::page_table()`](../../doc/lib/memory/fn.page_table.html).
На записях
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html),
которые доступны для пользовательского кода,
либо рекурсивно спускается на следующий уровень таблицы страниц,
либо на листьевом уровне копирует содержимое отображённой страницы.
Адрес отображаемой страницы вычисляет поэтапно на рекурсивных вызовах с помощью аргумента `virt`,
номера обрабатываемой записи
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html),
и уровня таблицы страниц.

Для копирования страницы в целевой процесс, сначала выделяет временную страницу
в собственном адресном пространстве с помощью функции
[`lib::memory::temp_page()`](../../doc/lib/memory/fn.temp_page.html).
Копирует текущую отображаемую страницу туда с помощью
[реализованной вами ранее](../../lab/book/5-um-3-eager-fork.html#copy_page)
функции
[`lib::memory::copy_page()`](../../doc/lib/memory/fn.copy_page.html).
Затем, с помощью системных вызовов
[`lib::syscall::copy_mapping()`](../../doc/lib/syscall/fn.copy_mapping.html)
и
[`lib::syscall::unmap()`](../../doc/lib/syscall/fn.unmap.html)
передаёт скопированную временную страницу потомку `child`, отображая её в его адресном пространстве
по адресу исходной страницы в своём адресном пространстве.

Запись номер `ku::process_info().recursive_mapping()` корневой таблицы страниц, а также страницы,
предоставляющие пользовательскому процессу информацию о нём, системе и
[`RingBuffer`](../../doc/ku/sync/pipe/struct.RingBuffer.html)
для логирования,
нужно проигнорировать.
То есть, страницы для которых `ku::process_info().contains_address()` возвращает `true`.
Их отобразит в память потомка ядро, выбирая новые, а не разделяемые, физические фреймы.


#### `eager_fork()`

```rust
fn eager_fork() -> Result<bool>
```

Создаёт процесс потомка с помощью
[реализованного вами ранее](../../lab/book/5-um-3-eager-fork.html#%D0%A1%D0%B8%D1%81%D1%82%D0%B5%D0%BC%D0%BD%D1%8B%D0%B9-%D0%B2%D1%8B%D0%B7%D0%BE%D0%B2-exofork)
системного вызова
[`lib::syscall::exofork()`](../../doc/lib/syscall/fn.exofork.html).
Далее копирует своё адресное пространство в пространство потомка с помощью функции `fn copy_address_space()`.
И запускает потомка системным вызовом
[`lib::syscall::set_state()`](../../doc/lib/syscall/fn.set_state.html),
устанавливая его состояние в
[`State::Runnable`](../../doc/ku/process/enum.State.html#variant.Runnable).
В потомке ничего не делает.
Возвращает `true` в процессе потомка и `false` в процессе родителя.


### Проверьте себя

Теперь должны заработать тесты `exofork_syscall()` и `eager_fork()` в файле
[`kernel/tests/5-um-3-eager-fork.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/5-um-3-eager-fork.rs):

```console
$ (cd kernel; cargo test --test 5-um-3-eager-fork)
...
5_um_3_eager_fork::t1_exofork_syscall-----------------------
20:54:40.213 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:54:40.223 0 I duplicate; address_space = "process" @ 0p7E2_6000
20:54:40.227 0 I switch to; address_space = "process" @ 0p7E2_6000
20:54:40.237 0 D extend mapping; block = [0v1000_0000, 0v1000_8C9C), size 35.152 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:54:40.245 0 D elf loadable program header; file_block = [0v20_4000, 0v20_CC9C), size 35.152 KiB; memory_block = [0v1000_0000, 0v1000_8C9C), size 35.152 KiB
20:54:40.275 0 D extend mapping; block = [0v1000_9000, 0v1005_557B), size 305.370 KiB; page_block = [0v1000_9000, 0v1005_6000), size 308.000 KiB
20:54:40.283 0 D elf loadable program header; file_block = [0v20_CCA0, 0v25_957B), size 306.214 KiB; memory_block = [0v1000_8CA0, 0v1005_557B), size 306.214 KiB
20:54:40.297 0 D elf loadable program header; file_block = [0v25_9580, 0v25_9650), size 208 B; memory_block = [0v1005_5580, 0v1005_5650), size 208 B
20:54:40.305 0 D extend mapping; block = [0v1005_6000, 0v1005_C9A0), size 26.406 KiB; page_block = [0v1005_6000, 0v1005_D000), size 28.000 KiB
20:54:40.311 0 D elf loadable program header; file_block = [0v25_9650, 0v26_0978), size 28.789 KiB; memory_block = [0v1005_5650, 0v1005_C9A0), size 28.828 KiB
20:54:40.343 0 I switch to; address_space = "base" @ 0p1000
20:54:40.347 0 I loaded ELF file; context = { rip: 0v1000_9E40, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.701 MiB; process = { pid: <current>, address_space: "process" @ 0p7E2_6000, { rip: 0v1000_9E40, rsp: 0v7F7F_FFFF_F000 } }
20:54:40.361 0 I allocate; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_6000, { rip: 0v1000_9E40, rsp: 0v7F7F_FFFF_F000 } }; process_count = 1
20:54:40.371 0 I user process page table entry; entry_point = 0v1000_9E40; frame = Frame(32259 @ 0p7E0_3000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:54:40.383 0 D process_frames = 158
20:54:41.007 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:54:41.015 0 I duplicate; address_space = "process" @ 0p7D8_8000
20:54:41.017 0 I switch to; address_space = "process" @ 0p7D8_8000
20:54:41.023 0 D extend mapping; block = [0v1000_0000, 0v1000_8C9C), size 35.152 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:54:41.031 0 D elf loadable program header; file_block = [0v20_4000, 0v20_CC9C), size 35.152 KiB; memory_block = [0v1000_0000, 0v1000_8C9C), size 35.152 KiB
20:54:41.061 0 D extend mapping; block = [0v1000_9000, 0v1005_557B), size 305.370 KiB; page_block = [0v1000_9000, 0v1005_6000), size 308.000 KiB
20:54:41.069 0 D elf loadable program header; file_block = [0v20_CCA0, 0v25_957B), size 306.214 KiB; memory_block = [0v1000_8CA0, 0v1005_557B), size 306.214 KiB
20:54:41.083 0 D elf loadable program header; file_block = [0v25_9580, 0v25_9650), size 208 B; memory_block = [0v1005_5580, 0v1005_5650), size 208 B
20:54:41.091 0 D extend mapping; block = [0v1005_6000, 0v1005_C9A0), size 26.406 KiB; page_block = [0v1005_6000, 0v1005_D000), size 28.000 KiB
20:54:41.097 0 D elf loadable program header; file_block = [0v25_9650, 0v26_0978), size 28.789 KiB; memory_block = [0v1005_5650, 0v1005_C9A0), size 28.828 KiB
20:54:41.123 0 I switch to; address_space = "base" @ 0p1000
20:54:41.127 0 I loaded ELF file; context = { rip: 0v1000_9E40, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.701 MiB; process = { pid: <current>, address_space: "process" @ 0p7D8_8000, { rip: 0v1000_9E40, rsp: 0v7F7F_FFFF_F000 } }
20:54:41.137 0 I allocate; slot = Process { pid: 1:0, address_space: "1:0" @ 0p7D8_8000, { rip: 0v1000_9E40, rsp: 0v7F7F_FFFF_F000 } }; process_count = 2
20:54:41.143 0 I user process page table entry; entry_point = 0v1000_9E40; frame = Frame(32101 @ 0p7D6_5000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:54:41.153 0 D process_frames = 158
20:54:41.155 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:41.791 0 I page allocator init; free_page_count = 33688649728; block = [0v180_0000_0000, 0v7F00_0000_0000), size 125.500 TiB
20:54:41.799 0 I duplicate; address_space = "process" @ 0p7CE_A000
20:54:41.803 0 I switch to; address_space = "process" @ 0p7CE_A000
20:54:41.809 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:41.813 0 I allocate; slot = Process { pid: 2:0, address_space: "2:0" @ 0p7CE_A000, { rip: 0v0, rsp: 0v0 } }; process_count = 3
20:54:41.819 0 I syscall = "exofork"; process = 0:0; child = 2:0
20:54:41.823 0 D child_pid = 2:0
20:54:41.827 0 D child = { pid: 2:0, address_space: "2:0" @ 0p7CE_A000, { rip: 0v0, rsp: 0v0 } }
20:54:41.839 0 I free; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_6000, { rip: 0v1000_9E40, rsp: 0v7F7F_FFFF_F000 } }; process_count = 2
20:54:41.845 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 9, unlocks: 9, waits: 0 }
20:54:41.853 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 5, unlocks: 5, waits: 0 }
20:54:41.859 0 I switch to; address_space = "base" @ 0p1000
20:54:41.861 0 I drop the current address space; address_space = "0:0" @ 0p7E2_6000; switch_to = "base" @ 0p1000
20:54:42.825 0 I free; slot = Process { pid: 1:0, address_space: "1:0" @ 0p7D8_8000, { rip: 0v1000_9E40, rsp: 0v7F7F_FFFF_F000 } }; process_count = 1
20:54:42.831 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 5, unlocks: 5, waits: 0 }
20:54:42.837 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:54:42.843 0 I drop; address_space = "1:0" @ 0p7D8_8000
20:54:43.801 0 I dequeue; pid = Some(2:0)
20:54:43.803 0 I switch to; address_space = "2:0" @ 0p7CE_A000
20:54:43.817 0 D entering the user mode; pid = 2:0; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7EFFFFFF5000, { mode: user, cs:rip: 0x0023:0v0, ss:rsp: 0x001B:0v0, rflags: IF } }
20:54:43.841 0 I user mode trap; trap = "Page Fault"; number = 14; info = { address: 0v0, code: 0b10100 = non-present page | execute | user }; context = { mode: user, cs:rip: 0x0023:0v0, ss:rsp: 0x001B:0v0, rflags: IF }; pid = 2:0
20:54:43.861 0 I free; slot = Process { pid: 2:0, address_space: "2:0" @ 0p7CE_A000, { rip: 0v0, rsp: 0v0 } }; process_count = 0
20:54:43.871 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 7, unlocks: 7, waits: 0 }
20:54:43.879 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:54:43.887 0 I switch to; address_space = "base" @ 0p1000
20:54:43.889 0 I drop the current address space; address_space = "2:0" @ 0p7CE_A000; switch_to = "base" @ 0p1000
20:54:44.001 0 D leaving the user mode; pid = 2:0
20:54:44.003 0 I dequeue; pid = None
5_um_3_eager_fork::t1_exofork_syscall-------------- [passed]

5_um_3_eager_fork::t2_eager_fork----------------------------
20:54:44.553 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:54:44.561 0 I duplicate; address_space = "process" @ 0p7CD_3000
20:54:44.563 0 I switch to; address_space = "process" @ 0p7CD_3000
20:54:44.569 0 D extend mapping; block = [0v1000_0000, 0v1000_982C), size 38.043 KiB; page_block = [0v1000_0000, 0v1000_A000), size 40.000 KiB
20:54:44.577 0 D elf loadable program header; file_block = [0v8B_9000, 0v8C_282C), size 38.043 KiB; memory_block = [0v1000_0000, 0v1000_982C), size 38.043 KiB
20:54:45.007 0 D extend mapping; block = [0v1000_A000, 0v1005_C8D3), size 330.206 KiB; page_block = [0v1000_A000, 0v1005_D000), size 332.000 KiB
20:54:45.015 0 D elf loadable program header; file_block = [0v8C_2830, 0v91_58D3), size 332.159 KiB; memory_block = [0v1000_9830, 0v1005_C8D3), size 332.159 KiB
20:54:45.029 0 D elf loadable program header; file_block = [0v91_58D8, 0v91_59C8), size 240 B; memory_block = [0v1005_C8D8, 0v1005_C9C8), size 240 B
20:54:45.039 0 D extend mapping; block = [0v1005_D000, 0v1006_4810), size 30.016 KiB; page_block = [0v1005_D000, 0v1006_5000), size 32.000 KiB
20:54:45.045 0 D elf loadable program header; file_block = [0v91_59C8, 0v91_D7E8), size 31.531 KiB; memory_block = [0v1005_C9C8, 0v1006_4810), size 31.570 KiB
20:54:45.071 0 I switch to; address_space = "base" @ 0p1000
20:54:45.075 0 I loaded ELF file; context = { rip: 0v1001_1DB0, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.974 MiB; process = { pid: <current>, address_space: "process" @ 0p7CD_3000, { rip: 0v1001_1DB0, rsp: 0v7F7F_FFFF_F000 } }
20:54:45.085 0 I allocate; slot = Process { pid: 2:1, address_space: "2:1" @ 0p7CD_3000, { rip: 0v1001_1DB0, rsp: 0v7F7F_FFFF_F000 } }; process_count = 1
20:54:45.091 0 I user process page table entry; entry_point = 0v1001_1DB0; frame = Frame(32120 @ 0p7D7_8000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:54:45.101 0 D process_frames = 166
20:54:45.103 0 I dequeue; pid = Some(2:1)
20:54:45.107 0 I switch to; address_space = "2:1" @ 0p7CD_3000
20:54:45.119 0 D entering the user mode; pid = 2:1; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1001_1DB0, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags: IF } }
20:54:45.133 0 I name = "eager_fork *"; pedigree = [2:1]; len = 1; capacity = 3; pid = 2:1
20:54:45.641 0 I page allocator init; free_page_count = 33688649728; block = [0v180_0000_0000, 0v7F00_0000_0000), size 125.500 TiB
20:54:45.649 0 I duplicate; address_space = "process" @ 0p7D5_C000
20:54:45.651 0 I switch to; address_space = "process" @ 0p7D5_C000
20:54:45.659 0 I switch to; address_space = "2:1" @ 0p7CD_3000
20:54:45.663 0 I allocate; slot = Process { pid: 1:1, address_space: "1:1" @ 0p7D5_C000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_DC48 } }; process_count = 2
20:54:45.669 0 I syscall = "exofork"; process = 2:1; child = 1:1
20:54:45.675 0 I syscall::exofork() done; child = Ok(1:1); pid = 2:1
20:54:46.263 0 I syscall::set_state(); child = 1:1; result = Ok(()); pid = 2:1
20:54:46.743 0 I page allocator init; free_page_count = 33688649728; block = [0v180_0000_0000, 0v7F00_0000_0000), size 125.500 TiB
20:54:46.749 0 I duplicate; address_space = "process" @ 0p7E0_5000
20:54:46.753 0 I switch to; address_space = "process" @ 0p7E0_5000
20:54:46.759 0 I switch to; address_space = "2:1" @ 0p7CD_3000
20:54:46.763 0 I allocate; slot = Process { pid: 0:1, address_space: "0:1" @ 0p7E0_5000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_DC48 } }; process_count = 3
20:54:46.771 0 I syscall = "exofork"; process = 2:1; child = 0:1
20:54:46.773 0 I syscall::exofork() done; child = Ok(0:1); pid = 2:1
20:54:47.343 0 I syscall::set_state(); child = 0:1; result = Ok(()); pid = 2:1
20:54:47.827 0 I page allocator init; free_page_count = 33688649728; block = [0v180_0000_0000, 0v7F00_0000_0000), size 125.500 TiB
20:54:47.835 0 I duplicate; address_space = "process" @ 0p7C2_C000
20:54:47.839 0 I switch to; address_space = "process" @ 0p7C2_C000
20:54:47.845 0 I switch to; address_space = "2:1" @ 0p7CD_3000
20:54:47.849 0 I allocate; slot = Process { pid: 3:0, address_space: "3:0" @ 0p7C2_C000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_DC48 } }; process_count = 4
20:54:47.855 0 I syscall = "exofork"; process = 2:1; child = 3:0
20:54:47.859 0 I syscall::exofork() done; child = Ok(3:0); pid = 2:1
20:54:48.429 0 I syscall::set_state(); child = 3:0; result = Ok(()); pid = 2:1
20:54:48.435 0 I free; slot = Process { pid: 2:1, address_space: "2:1" @ 0p7CD_3000, { rip: 0v1001_1DB0, rsp: 0v7F7F_FFFF_F000 } }; process_count = 3
20:54:48.443 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 1595, unlocks: 1595, waits: 0 }
20:54:48.449 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 1197, unlocks: 1197, waits: 0 }
20:54:48.455 0 I switch to; address_space = "base" @ 0p1000
20:54:48.457 0 I drop the current address space; address_space = "2:1" @ 0p7CD_3000; switch_to = "base" @ 0p1000
20:54:49.035 0 I syscall = "exit"; pid = 2:1; code = 0; reason = Ok(Ok)
20:54:49.039 0 D leaving the user mode; pid = 2:1
20:54:49.041 0 I dequeue; pid = Some(1:1)
20:54:49.045 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:54:49.055 0 D entering the user mode; pid = 1:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7EFFFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_DC48, rflags: IF } }
20:54:49.073 0 D leaving the user mode; pid = 1:1
20:54:49.075 0 I the process was preempted; pid = 1:1; user_context = { mode: user, cs:rip: 0x0023:0v1001_73D5, ss:rsp: 0x001B:0v7F7F_FFFF_D4A8, rflags: IF }
20:54:49.085 0 I dequeue; pid = Some(0:1)
20:54:49.089 0 I switch to; address_space = "0:1" @ 0p7E0_5000
20:54:49.101 0 D entering the user mode; pid = 0:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7EFFFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_DC48, rflags: IF } }
20:54:49.111 0 I syscall::exofork() done; child = Ok(<current>); pid = 0:1
20:54:49.127 0 I just created; child = <current>; pid = 0:1; pid = 0:1
20:54:49.129 0 I name = "eager_fork *1"; pedigree = [2:1, 0:1]; len = 2; capacity = 3; pid = 0:1
20:54:49.631 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:54:49.637 0 I duplicate; address_space = "process" @ 0p7CD_3000
20:54:49.641 0 I switch to; address_space = "process" @ 0p7CD_3000
20:54:49.647 0 I switch to; address_space = "0:1" @ 0p7E0_5000
20:54:49.651 0 I allocate; slot = Process { pid: 2:2, address_space: "2:2" @ 0p7CD_3000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 4
20:54:49.659 0 I syscall = "exofork"; process = 0:1; child = 2:2
20:54:49.663 0 I syscall::exofork() done; child = Ok(2:2); pid = 0:1
20:54:50.071 0 D leaving the user mode; pid = 0:1
20:54:50.075 0 I the process was preempted; pid = 0:1; user_context = { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF }
20:54:50.087 0 I dequeue; pid = Some(3:0)
20:54:50.089 0 I switch to; address_space = "3:0" @ 0p7C2_C000
20:54:50.101 0 D entering the user mode; pid = 3:0; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7EFFFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_DC48, rflags: IF } }
20:54:50.113 0 I syscall::exofork() done; child = Ok(<current>); pid = 3:0
20:54:50.127 0 I just created; child = <current>; pid = 3:0; pid = 3:0
20:54:50.131 0 I name = "eager_fork *2"; pedigree = [2:1, 3:0]; len = 2; capacity = 3; pid = 3:0
20:54:50.669 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:54:50.677 0 I duplicate; address_space = "process" @ 0p7CF_B000
20:54:50.681 0 I switch to; address_space = "process" @ 0p7CF_B000
20:54:50.687 0 I switch to; address_space = "3:0" @ 0p7C2_C000
20:54:50.691 0 I allocate; slot = Process { pid: 4:0, address_space: "4:0" @ 0p7CF_B000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 5
20:54:50.699 0 I syscall = "exofork"; process = 3:0; child = 4:0
20:54:50.701 0 I syscall::exofork() done; child = Ok(4:0); pid = 3:0
20:54:51.285 0 I syscall::set_state(); child = 4:0; result = Ok(()); pid = 3:0
20:54:51.779 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:54:51.787 0 I duplicate; address_space = "process" @ 0p7B0_2000
20:54:51.789 0 I switch to; address_space = "process" @ 0p7B0_2000
20:54:51.797 0 I switch to; address_space = "3:0" @ 0p7C2_C000
20:54:51.801 0 I allocate; slot = Process { pid: 5:0, address_space: "5:0" @ 0p7B0_2000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 6
20:54:51.807 0 I syscall = "exofork"; process = 3:0; child = 5:0
20:54:51.811 0 I syscall::exofork() done; child = Ok(5:0); pid = 3:0
20:54:52.391 0 I syscall::set_state(); child = 5:0; result = Ok(()); pid = 3:0
20:54:52.887 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:54:52.895 0 I duplicate; address_space = "process" @ 0p7A5_4000
20:54:52.899 0 I switch to; address_space = "process" @ 0p7A5_4000
20:54:52.905 0 I switch to; address_space = "3:0" @ 0p7C2_C000
20:54:52.909 0 I allocate; slot = Process { pid: 6:0, address_space: "6:0" @ 0p7A5_4000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 7
20:54:52.915 0 I syscall = "exofork"; process = 3:0; child = 6:0
20:54:52.919 0 I syscall::exofork() done; child = Ok(6:0); pid = 3:0
20:54:53.499 0 I syscall::set_state(); child = 6:0; result = Ok(()); pid = 3:0
20:54:53.505 0 I free; slot = Process { pid: 3:0, address_space: "3:0" @ 0p7C2_C000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_DC48 } }; process_count = 6
20:54:53.511 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 1727, unlocks: 1727, waits: 0 }
20:54:53.517 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 1328, unlocks: 1328, waits: 0 }
20:54:53.525 0 I switch to; address_space = "base" @ 0p1000
20:54:53.527 0 I drop the current address space; address_space = "3:0" @ 0p7C2_C000; switch_to = "base" @ 0p1000
20:54:54.123 0 I syscall = "exit"; pid = 3:0; code = 0; reason = Ok(Ok)
20:54:54.127 0 D leaving the user mode; pid = 3:0
20:54:54.129 0 I dequeue; pid = Some(1:1)
20:54:54.133 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:54:54.143 0 D entering the user mode; pid = 1:1; registers = { rax: 0x7F7FFFFFD4C0, rdi: 0x7EFFFFFF7003, rsi: 0x7F7FFFFFD4E7, { mode: user, cs:rip: 0x0023:0v1001_73D5, ss:rsp: 0x001B:0v7F7F_FFFF_D4A8, rflags: IF } }
20:54:49.065 0 I syscall::exofork() done; child = Ok(<current>); pid = 1:1
20:54:54.163 0 I just created; child = <current>; pid = 1:1; pid = 1:1
20:54:54.165 0 I name = "eager_fork *0"; pedigree = [2:1, 1:1]; len = 2; capacity = 3; pid = 1:1
20:54:54.665 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:54:54.673 0 I duplicate; address_space = "process" @ 0p7C2_C000
20:54:54.675 0 I switch to; address_space = "process" @ 0p7C2_C000
20:54:54.683 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:54:54.687 0 I allocate; slot = Process { pid: 3:1, address_space: "3:1" @ 0p7C2_C000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 7
20:54:54.693 0 I syscall = "exofork"; process = 1:1; child = 3:1
20:54:54.697 0 I syscall::exofork() done; child = Ok(3:1); pid = 1:1
20:54:54.971 0 D leaving the user mode; pid = 1:1
20:54:54.975 0 I the process was preempted; pid = 1:1; user_context = { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF }
20:54:54.987 0 I dequeue; pid = Some(0:1)
20:54:54.989 0 I switch to; address_space = "0:1" @ 0p7E0_5000
20:54:55.001 0 D entering the user mode; pid = 0:1; registers = { rax: 0xC00, rdi: 0x7EFFFFF52000, rsi: 0x10051000, { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF } }
20:54:55.271 0 D leaving the user mode; pid = 0:1
20:54:55.273 0 I the process was preempted; pid = 0:1; user_context = { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF }
20:54:55.285 0 I dequeue; pid = Some(4:0)
20:54:55.289 0 I switch to; address_space = "4:0" @ 0p7CF_B000
20:54:55.299 0 D entering the user mode; pid = 4:0; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D778, rflags: IF } }
20:54:55.311 0 I syscall::exofork() done; child = Ok(<current>); pid = 4:0
20:54:55.325 0 I just created; child = <current>; pid = 4:0; pid = 4:0
20:54:55.327 0 I name = "eager_fork *20"; pedigree = [2:1, 3:0, 4:0]; len = 3; capacity = 3; pid = 4:0
20:54:55.345 0 I free; slot = Process { pid: 4:0, address_space: "4:0" @ 0p7CF_B000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 6
20:54:55.353 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 137, unlocks: 137, waits: 0 }
20:54:55.359 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 134, unlocks: 134, waits: 0 }
20:54:55.365 0 I switch to; address_space = "base" @ 0p1000
20:54:55.369 0 I drop the current address space; address_space = "4:0" @ 0p7CF_B000; switch_to = "base" @ 0p1000
20:54:55.969 0 I syscall = "exit"; pid = 4:0; code = 0; reason = Ok(Ok)
20:54:55.973 0 D leaving the user mode; pid = 4:0
20:54:55.975 0 I dequeue; pid = Some(5:0)
20:54:55.979 0 I switch to; address_space = "5:0" @ 0p7B0_2000
20:54:55.991 0 D entering the user mode; pid = 5:0; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D778, rflags: IF } }
20:54:56.003 0 I syscall::exofork() done; child = Ok(<current>); pid = 5:0
20:54:56.017 0 I just created; child = <current>; pid = 5:0; pid = 5:0
20:54:56.019 0 I name = "eager_fork *21"; pedigree = [2:1, 3:0, 5:0]; len = 3; capacity = 3; pid = 5:0
20:54:56.037 0 I free; slot = Process { pid: 5:0, address_space: "5:0" @ 0p7B0_2000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 5
20:54:56.045 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 137, unlocks: 137, waits: 0 }
20:54:56.051 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 134, unlocks: 134, waits: 0 }
20:54:56.057 0 I switch to; address_space = "base" @ 0p1000
20:54:56.059 0 I drop the current address space; address_space = "5:0" @ 0p7B0_2000; switch_to = "base" @ 0p1000
20:54:56.657 0 I syscall = "exit"; pid = 5:0; code = 0; reason = Ok(Ok)
20:54:56.661 0 D leaving the user mode; pid = 5:0
20:54:56.665 0 I dequeue; pid = Some(6:0)
20:54:56.667 0 I switch to; address_space = "6:0" @ 0p7A5_4000
20:54:56.679 0 D entering the user mode; pid = 6:0; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D778, rflags: IF } }
20:54:56.691 0 I syscall::exofork() done; child = Ok(<current>); pid = 6:0
20:54:56.703 0 I just created; child = <current>; pid = 6:0; pid = 6:0
20:54:56.707 0 I name = "eager_fork *22"; pedigree = [2:1, 3:0, 6:0]; len = 3; capacity = 3; pid = 6:0
20:54:56.725 0 I free; slot = Process { pid: 6:0, address_space: "6:0" @ 0p7A5_4000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 4
20:54:56.733 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 137, unlocks: 137, waits: 0 }
20:54:56.739 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 134, unlocks: 134, waits: 0 }
20:54:56.745 0 I switch to; address_space = "base" @ 0p1000
20:54:56.747 0 I drop the current address space; address_space = "6:0" @ 0p7A5_4000; switch_to = "base" @ 0p1000
20:54:57.353 0 I syscall = "exit"; pid = 6:0; code = 0; reason = Ok(Ok)
20:54:57.357 0 D leaving the user mode; pid = 6:0
20:54:57.359 0 I dequeue; pid = Some(1:1)
20:54:57.363 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:54:57.375 0 D entering the user mode; pid = 1:1; registers = { rax: 0x0, rdi: 0x7EFFFFF80000, rsi: 0x1003A000, { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF } }
20:54:57.571 0 D leaving the user mode; pid = 1:1
20:54:57.575 0 I the process was preempted; pid = 1:1; user_context = { mode: user, cs:rip: 0x0023:0v1002_0992, ss:rsp: 0x001B:0v7F7F_FFFF_C328, rflags: IF PF }
20:54:57.585 0 I dequeue; pid = Some(0:1)
20:54:57.587 0 I switch to; address_space = "0:1" @ 0p7E0_5000
20:54:57.599 0 D entering the user mode; pid = 0:1; registers = { rax: 0xD00, rdi: 0x7EFFFFEFA000, rsi: 0x7F7FFFFF8000, { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF } }
20:54:57.639 0 I syscall::set_state(); child = 2:2; result = Ok(()); pid = 0:1
20:54:58.129 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:54:58.137 0 I duplicate; address_space = "process" @ 0p7A4_1000
20:54:58.139 0 I switch to; address_space = "process" @ 0p7A4_1000
20:54:58.149 0 I switch to; address_space = "0:1" @ 0p7E0_5000
20:54:58.153 0 I allocate; slot = Process { pid: 6:1, address_space: "6:1" @ 0p7A4_1000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 5
20:54:58.159 0 I syscall = "exofork"; process = 0:1; child = 6:1
20:54:58.163 0 I syscall::exofork() done; child = Ok(6:1); pid = 0:1
20:54:58.371 0 D leaving the user mode; pid = 0:1
20:54:58.375 0 I the process was preempted; pid = 0:1; user_context = { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF }
20:54:58.387 0 I dequeue; pid = Some(1:1)
20:54:58.389 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:54:58.401 0 D entering the user mode; pid = 1:1; registers = { rax: 0x8000000000000FFF, rdi: 0x7F7FFFFFC338, rsi: 0x11A, { mode: user, cs:rip: 0x0023:0v1002_0992, ss:rsp: 0x001B:0v7F7F_FFFF_C328, rflags: IF PF } }
20:54:58.571 0 D leaving the user mode; pid = 1:1
20:54:58.575 0 I the process was preempted; pid = 1:1; user_context = { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF }
20:54:58.587 0 I dequeue; pid = Some(2:2)
20:54:58.589 0 I switch to; address_space = "2:2" @ 0p7CD_3000
20:54:58.601 0 D entering the user mode; pid = 2:2; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D778, rflags: IF } }
20:54:58.613 0 I syscall::exofork() done; child = Ok(<current>); pid = 2:2
20:54:58.627 0 I just created; child = <current>; pid = 2:2; pid = 2:2
20:54:58.631 0 I name = "eager_fork *10"; pedigree = [2:1, 0:1, 2:2]; len = 3; capacity = 3; pid = 2:2
20:54:58.649 0 I free; slot = Process { pid: 2:2, address_space: "2:2" @ 0p7CD_3000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 4
20:54:58.655 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 137, unlocks: 137, waits: 0 }
20:54:58.661 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 134, unlocks: 134, waits: 0 }
20:54:58.667 0 I switch to; address_space = "base" @ 0p1000
20:54:58.671 0 I drop the current address space; address_space = "2:2" @ 0p7CD_3000; switch_to = "base" @ 0p1000
20:54:59.267 0 I syscall = "exit"; pid = 2:2; code = 0; reason = Ok(Ok)
20:54:59.271 0 D leaving the user mode; pid = 2:2
20:54:59.275 0 I dequeue; pid = Some(0:1)
20:54:59.277 0 I switch to; address_space = "0:1" @ 0p7E0_5000
20:54:59.289 0 D entering the user mode; pid = 0:1; registers = { rax: 0xD00, rdi: 0x7EFFFFE9C000, rsi: 0x10028000, { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF } }
20:54:59.671 0 D leaving the user mode; pid = 0:1
20:54:59.675 0 I the process was preempted; pid = 0:1; user_context = { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF }
20:54:59.687 0 I dequeue; pid = Some(1:1)
20:54:59.689 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:54:59.701 0 D entering the user mode; pid = 1:1; registers = { rax: 0xA00, rdi: 0x7EFFFFEF6000, rsi: 0x7F7FFFFFA000, { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF } }
20:54:59.737 0 I syscall::set_state(); child = 3:1; result = Ok(()); pid = 1:1
20:55:00.237 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:00.243 0 I duplicate; address_space = "process" @ 0p7D0_4000
20:55:00.247 0 I switch to; address_space = "process" @ 0p7D0_4000
20:55:00.257 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:55:00.261 0 I allocate; slot = Process { pid: 2:3, address_space: "2:3" @ 0p7D0_4000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 5
20:55:00.267 0 I syscall = "exofork"; process = 1:1; child = 2:3
20:55:00.271 0 D leaving the user mode; pid = 1:1
20:55:00.275 0 I the process was preempted; pid = 1:1; user_context = { mode: user, cs:rip: 0x0023:0v1001_F330, ss:rsp: 0x001B:0v7F7F_FFFF_BBF0, rflags: IF }
20:55:00.285 0 I dequeue; pid = Some(0:1)
20:55:00.287 0 I switch to; address_space = "0:1" @ 0p7E0_5000
20:55:00.299 0 D entering the user mode; pid = 0:1; registers = { rax: 0xB00, rdi: 0x7EFFFFDF4000, rsi: 0x7F7FFFFF7000, { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF } }
20:55:00.355 0 I syscall::set_state(); child = 6:1; result = Ok(()); pid = 0:1
20:55:00.863 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:00.871 0 I duplicate; address_space = "process" @ 0p7D2_B000
20:55:00.873 0 I switch to; address_space = "process" @ 0p7D2_B000
20:55:00.881 0 I switch to; address_space = "0:1" @ 0p7E0_5000
20:55:00.885 0 I allocate; slot = Process { pid: 5:1, address_space: "5:1" @ 0p7D2_B000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 6
20:55:00.891 0 I syscall = "exofork"; process = 0:1; child = 5:1
20:55:00.895 0 I syscall::exofork() done; child = Ok(5:1); pid = 0:1
20:55:01.471 0 D leaving the user mode; pid = 0:1
20:55:01.475 0 I the process was preempted; pid = 0:1; user_context = { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF }
20:55:01.487 0 I dequeue; pid = Some(3:1)
20:55:01.489 0 I switch to; address_space = "3:1" @ 0p7C2_C000
20:55:01.501 0 D entering the user mode; pid = 3:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D778, rflags: IF } }
20:55:01.511 0 I syscall::exofork() done; child = Ok(<current>); pid = 3:1
20:55:01.525 0 I just created; child = <current>; pid = 3:1; pid = 3:1
20:55:01.527 0 I name = "eager_fork *00"; pedigree = [2:1, 1:1, 3:1]; len = 3; capacity = 3; pid = 3:1
20:55:01.545 0 I free; slot = Process { pid: 3:1, address_space: "3:1" @ 0p7C2_C000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 5
20:55:01.553 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 137, unlocks: 137, waits: 0 }
20:55:01.559 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 134, unlocks: 134, waits: 0 }
20:55:01.565 0 I switch to; address_space = "base" @ 0p1000
20:55:01.567 0 I drop the current address space; address_space = "3:1" @ 0p7C2_C000; switch_to = "base" @ 0p1000
20:55:02.159 0 I syscall = "exit"; pid = 3:1; code = 0; reason = Ok(Ok)
20:55:02.163 0 D leaving the user mode; pid = 3:1
20:55:02.167 0 I dequeue; pid = Some(1:1)
20:55:02.171 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:55:02.181 0 D entering the user mode; pid = 1:1; registers = { rax: 0x7EFFFFFF72C1, rdi: 0x7F7FFFFFBC28, rsi: 0x7EFFFFFF72C1, { mode: user, cs:rip: 0x0023:0v1001_F330, ss:rsp: 0x001B:0v7F7F_FFFF_BBF0, rflags: IF } }
20:55:00.271 0 I syscall::exofork() done; child = Ok(2:3); pid = 1:1
20:55:02.811 0 I syscall::set_state(); child = 2:3; result = Ok(()); pid = 1:1
20:55:03.321 0 I page allocator init; free_page_count = 33554432000; block = [0v180_0000_0000, 0v7E80_0000_0000), size 125.000 TiB
20:55:03.329 0 I duplicate; address_space = "process" @ 0p7BA_A000
20:55:03.331 0 I switch to; address_space = "process" @ 0p7BA_A000
20:55:03.339 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:55:03.343 0 I allocate; slot = Process { pid: 3:2, address_space: "3:2" @ 0p7BA_A000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 6
20:55:03.351 0 I syscall = "exofork"; process = 1:1; child = 3:2
20:55:03.353 0 I syscall::exofork() done; child = Ok(3:2); pid = 1:1
20:55:03.571 0 D leaving the user mode; pid = 1:1
20:55:03.575 0 I the process was preempted; pid = 1:1; user_context = { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF }
20:55:03.587 0 I dequeue; pid = Some(6:1)
20:55:03.589 0 I switch to; address_space = "6:1" @ 0p7A4_1000
20:55:03.601 0 D entering the user mode; pid = 6:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D778, rflags: IF ZF PF } }
20:55:03.611 0 I syscall::exofork() done; child = Ok(<current>); pid = 6:1
20:55:03.625 0 I just created; child = <current>; pid = 6:1; pid = 6:1
20:55:03.629 0 I name = "eager_fork *11"; pedigree = [2:1, 0:1, 6:1]; len = 3; capacity = 3; pid = 6:1
20:55:03.647 0 I free; slot = Process { pid: 6:1, address_space: "6:1" @ 0p7A4_1000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 5
20:55:03.653 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 137, unlocks: 137, waits: 0 }
20:55:03.659 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 134, unlocks: 134, waits: 0 }
20:55:03.665 0 I switch to; address_space = "base" @ 0p1000
20:55:03.669 0 I drop the current address space; address_space = "6:1" @ 0p7A4_1000; switch_to = "base" @ 0p1000
20:55:04.261 0 I syscall = "exit"; pid = 6:1; code = 0; reason = Ok(Ok)
20:55:04.265 0 D leaving the user mode; pid = 6:1
20:55:04.267 0 I dequeue; pid = Some(0:1)
20:55:04.271 0 I switch to; address_space = "0:1" @ 0p7E0_5000
20:55:04.281 0 D entering the user mode; pid = 0:1; registers = { rax: 0xF00, rdi: 0x7EFFFFCE6000, rsi: 0x7F7FFFFFA000, { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF } }
20:55:04.313 0 I syscall::set_state(); child = 5:1; result = Ok(()); pid = 0:1
20:55:04.319 0 I free; slot = Process { pid: 0:1, address_space: "0:1" @ 0p7E0_5000, { rip: 0v1003_2585, rsp: 0v7F7F_FFFF_C258 } }; process_count = 4
20:55:04.327 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 1737, unlocks: 1737, waits: 0 }
20:55:04.333 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 1328, unlocks: 1328, waits: 0 }
20:55:04.339 0 I switch to; address_space = "base" @ 0p1000
20:55:04.341 0 I drop the current address space; address_space = "0:1" @ 0p7E0_5000; switch_to = "base" @ 0p1000
20:55:04.927 0 I syscall = "exit"; pid = 0:1; code = 0; reason = Ok(Ok)
20:55:04.931 0 D leaving the user mode; pid = 0:1
20:55:04.933 0 I dequeue; pid = Some(2:3)
20:55:04.937 0 I switch to; address_space = "2:3" @ 0p7D0_4000
20:55:04.947 0 D entering the user mode; pid = 2:3; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D778, rflags: IF ZF PF } }
20:55:04.971 0 D leaving the user mode; pid = 2:3
20:55:04.975 0 I the process was preempted; pid = 2:3; user_context = { mode: user, cs:rip: 0x0023:0v1001_FCD0, ss:rsp: 0x001B:0v7F7F_FFFF_D4F0, rflags: IF PF }
20:55:04.985 0 I dequeue; pid = Some(1:1)
20:55:04.987 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:55:04.999 0 D entering the user mode; pid = 1:1; registers = { rax: 0xD00, rdi: 0x7EFFFFD9A000, rsi: 0x10025000, { mode: user, cs:rip: 0x0023:0v1003_2585, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF ZF PF } }
20:55:05.371 0 D leaving the user mode; pid = 1:1
20:55:05.375 0 I the process was preempted; pid = 1:1; user_context = { mode: user, cs:rip: 0x0023:0v1003_254C, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF SF CF }
20:55:05.387 0 I dequeue; pid = Some(5:1)
20:55:05.389 0 I switch to; address_space = "5:1" @ 0p7D2_B000
20:55:05.401 0 D entering the user mode; pid = 5:1; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D778, rflags: IF ZF PF } }
20:55:05.413 0 I syscall::exofork() done; child = Ok(<current>); pid = 5:1
20:55:05.425 0 I just created; child = <current>; pid = 5:1; pid = 5:1
20:55:05.429 0 I name = "eager_fork *12"; pedigree = [2:1, 0:1, 5:1]; len = 3; capacity = 3; pid = 5:1
20:55:05.447 0 I free; slot = Process { pid: 5:1, address_space: "5:1" @ 0p7D2_B000, { rip: 0v1000_B0F5, rsp: 0v7F7F_FFFF_D778 } }; process_count = 3
20:55:05.455 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 137, unlocks: 137, waits: 0 }
20:55:05.461 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 134, unlocks: 134, waits: 0 }
20:55:05.467 0 I switch to; address_space = "base" @ 0p1000
20:55:05.469 0 I drop the current address space; address_space = "5:1" @ 0p7D2_B000; switch_to = "base" @ 0p1000
20:55:06.067 0 I syscall = "exit"; pid = 5:1; code = 0; reason = Ok(Ok)
20:55:06.071 0 D leaving the user mode; pid = 5:1
20:55:06.075 0 I dequeue; pid = Some(2:3)
20:55:06.077 0 I switch to; address_space = "2:3" @ 0p7D0_4000
20:55:06.089 0 D entering the user mode; pid = 2:3; registers = { rax: 0x0, rdi: 0x7F7FFFFFD5B8, rsi: 0x8, { mode: user, cs:rip: 0x0023:0v1001_FCD0, ss:rsp: 0x001B:0v7F7F_FFFF_D4F0, rflags: IF PF } }
20:55:04.959 0 I syscall::exofork() done; child = Ok(<current>); pid = 2:3
20:55:06.099 0 I just created; child = <current>; pid = 2:3; pid = 2:3
20:55:06.103 0 I name = "eager_fork *01"; pedigree = [2:1, 1:1, 2:3]; len = 3; capacity = 3; pid = 2:3
20:55:06.121 0 I free; slot = Process { pid: 2:3, address_space: "2:3" @ 0p7D0_4000, { rip: 0v1001_FCD0, rsp: 0v7F7F_FFFF_D4F0 } }; process_count = 2
20:55:06.127 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 139, unlocks: 139, waits: 0 }
20:55:06.133 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 134, unlocks: 134, waits: 0 }
20:55:06.139 0 I switch to; address_space = "base" @ 0p1000
20:55:06.143 0 I drop the current address space; address_space = "2:3" @ 0p7D0_4000; switch_to = "base" @ 0p1000
20:55:06.735 0 I syscall = "exit"; pid = 2:3; code = 0; reason = Ok(Ok)
20:55:06.737 0 D leaving the user mode; pid = 2:3
20:55:06.741 0 I dequeue; pid = Some(1:1)
20:55:06.743 0 I switch to; address_space = "1:1" @ 0p7D5_C000
20:55:06.755 0 D entering the user mode; pid = 1:1; registers = { rax: 0x1000, rdi: 0x7EFFFFCF6000, rsi: 0x7F7FFFFF2000, { mode: user, cs:rip: 0x0023:0v1003_254C, ss:rsp: 0x001B:0v7F7F_FFFF_C258, rflags: IF SF CF } }
20:55:06.821 0 I syscall::set_state(); child = 3:2; result = Ok(()); pid = 1:1
20:55:06.827 0 I free; slot = Process { pid: 1:1, address_space: "1:1" @ 0p7D5_C000, { rip: 0v1003_254C, rsp: 0v7F7F_FFFF_C258 } }; process_count = 1
20:55:06.833 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 1741, unlocks: 1741, waits: 0 }
20:55:06.839 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 1328, unlocks: 1328, waits: 0 }
20:55:06.845 0 I switch to; address_space = "base" @ 0p1000
20:55:06.849 0 I drop the current address space; address_space = "1:1" @ 0p7D5_C000; switch_to = "base" @ 0p1000
20:55:07.433 0 I syscall = "exit"; pid = 1:1; code = 0; reason = Ok(Ok)
20:55:07.437 0 D leaving the user mode; pid = 1:1
20:55:07.441 0 I dequeue; pid = Some(3:2)
20:55:07.443 0 I switch to; address_space = "3:2" @ 0p7BA_A000
20:55:07.455 0 D entering the user mode; pid = 3:2; registers = { rax: 0x0, rdi: 0xFFFFFFFFFFFFFFFE, rsi: 0x7E7FFFFF5000, { mode: user, cs:rip: 0x0023:0v1000_B0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D778, rflags: IF } }
20:55:07.471 0 D leaving the user mode; pid = 3:2
20:55:07.475 0 I the process was preempted; pid = 3:2; user_context = { mode: user, cs:rip: 0x0023:0v1001_75DF, ss:rsp: 0x001B:0v7F7F_FFFF_CFC8, rflags: IF PF }
20:55:07.485 0 I dequeue; pid = Some(3:2)
20:55:07.487 0 I switch to; address_space = "3:2" @ 0p7BA_A000
20:55:07.499 0 D entering the user mode; pid = 3:2; registers = { rax: 0x10, rdi: 0x10, rsi: 0x7F7FFFFFCF8E, { mode: user, cs:rip: 0x0023:0v1001_75DF, ss:rsp: 0x001B:0v7F7F_FFFF_CFC8, rflags: IF PF } }
20:55:07.467 0 I syscall::exofork() done; child = Ok(<current>); pid = 3:2
20:55:07.517 0 I just created; child = <current>; pid = 3:2; pid = 3:2
20:55:07.519 0 I name = "eager_fork *02"; pedigree = [2:1, 1:1, 3:2]; len = 3; capacity = 3; pid = 3:2
20:55:07.537 0 I free; slot = Process { pid: 3:2, address_space: "3:2" @ 0p7BA_A000, { rip: 0v1001_75DF, rsp: 0v7F7F_FFFF_CFC8 } }; process_count = 0
20:55:07.543 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 139, unlocks: 139, waits: 0 }
20:55:07.551 0 D dropping; spinlock = kernel/src/process/process.rs:94:28; stats = Stats { failures: 0, locks: 134, unlocks: 134, waits: 0 }
20:55:07.557 0 I switch to; address_space = "base" @ 0p1000
20:55:07.559 0 I drop the current address space; address_space = "3:2" @ 0p7BA_A000; switch_to = "base" @ 0p1000
20:55:08.155 0 I syscall = "exit"; pid = 3:2; code = 0; reason = Ok(Ok)
20:55:08.159 0 D leaving the user mode; pid = 3:2
20:55:08.163 0 I dequeue; pid = None
20:55:08.171 0 D parent = 0:1; process = 2:2
20:55:08.177 0 D parent = 0:1; process = 5:1
20:55:08.181 0 D parent = 0:1; process = 6:1
20:55:08.185 0 D parent = 1:1; process = 2:3
20:55:08.189 0 D parent = 1:1; process = 3:1
20:55:08.193 0 D parent = 1:1; process = 3:2
20:55:08.195 0 D parent = 2:1; process = 0:1
20:55:08.199 0 D parent = 2:1; process = 1:1
20:55:08.203 0 D parent = 2:1; process = 3:0
20:55:08.205 0 D parent = 3:0; process = 4:0
20:55:08.209 0 D parent = 3:0; process = 5:0
20:55:08.213 0 D parent = 3:0; process = 6:0
20:55:08.217 0 D graphviz = digraph process_tree { node [ style = filled; fillcolor = "#CCCCCC"]; "0:1" -> "2:2"; "0:1" -> "5:1"; "0:1" -> "6:1"; "1:1" -> "2:3"; "1:1" -> "3:1"; "1:1" -> "3:2"; "2:1" -> "0:1"; "2:1" -> "1:1"; "2:1" -> "3:0"; "3:0" -> "4:0"; "3:0" -> "5:0"; "3:0" -> "6:0"; }
5_um_3_eager_fork::t2_eager_fork------------------- [passed]
20:55:08.231 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```

В этом запуске видно, что корневым явился процесс с идентификатором `2:1`:

```console
$ grep pedigree log
20:54:45.133 0 I name = "eager_fork *"; pedigree = [2:1]; len = 1; capacity = 3; pid = 2:1
20:54:49.129 0 I name = "eager_fork *1"; pedigree = [2:1, 0:1]; len = 2; capacity = 3; pid = 0:1
20:54:50.131 0 I name = "eager_fork *2"; pedigree = [2:1, 3:0]; len = 2; capacity = 3; pid = 3:0
20:54:54.165 0 I name = "eager_fork *0"; pedigree = [2:1, 1:1]; len = 2; capacity = 3; pid = 1:1
20:54:55.327 0 I name = "eager_fork *20"; pedigree = [2:1, 3:0, 4:0]; len = 3; capacity = 3; pid = 4:0
20:54:56.019 0 I name = "eager_fork *21"; pedigree = [2:1, 3:0, 5:0]; len = 3; capacity = 3; pid = 5:0
20:54:56.707 0 I name = "eager_fork *22"; pedigree = [2:1, 3:0, 6:0]; len = 3; capacity = 3; pid = 6:0
20:54:58.631 0 I name = "eager_fork *10"; pedigree = [2:1, 0:1, 2:2]; len = 3; capacity = 3; pid = 2:2
20:55:01.527 0 I name = "eager_fork *00"; pedigree = [2:1, 1:1, 3:1]; len = 3; capacity = 3; pid = 3:1
20:55:03.629 0 I name = "eager_fork *11"; pedigree = [2:1, 0:1, 6:1]; len = 3; capacity = 3; pid = 6:1
20:55:05.429 0 I name = "eager_fork *12"; pedigree = [2:1, 0:1, 5:1]; len = 3; capacity = 3; pid = 5:1
20:55:06.103 0 I name = "eager_fork *01"; pedigree = [2:1, 1:1, 2:3]; len = 3; capacity = 3; pid = 2:3
20:55:07.519 0 I name = "eager_fork *02"; pedigree = [2:1, 1:1, 3:2]; len = 3; capacity = 3; pid = 3:2
```

Он запустил трёх потомков --- `0:1`, `3:0` и `1:1`.
А, например, родословная процесса `6:0` --- `pedigree = [2:1, 3:0, 6:0]`.

Получилось такое дерево процессов:
![](5-um-3-eager-fork.svg)


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/memory/mapping.rs  | 19 +++++++++++++++++--
 kernel/src/process/syscall.rs | 59 ++++++++++++++++++++++++++++++++++++++++++++++++++++-------
 user/eager_fork/src/main.rs   | 58 ++++++++++++++++++++++++++++++++++++++++++++++++++++++----
 user/lib/src/memory/mod.rs    | 40 ++++++++++++++++++++++++++++++++++------
 4 files changed, 157 insertions(+), 19 deletions(-)
```
