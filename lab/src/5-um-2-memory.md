## Системные вызовы для работы с виртуальной памятью

Добавим новые системные вызовы в файл [`kernel/src/process/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/syscall.rs).

Эти системные вызовы будут принимать идентификатор
[`Pid`](../../doc/ku/process/pid/enum.Pid.html)
целевого процесса.
То есть, они будут позволять вызывающему процессу выполнить действие над самим собой,
задав
[`Pid::Current`](../../doc/ku/process/pid/enum.Pid.html#variant.Current)
или явно собственный идентификатор
[`Pid::Id`](../../doc/ku/process/pid/enum.Pid.html#variant.Id).
Или же над другим процессом, указав его
[`Pid::Id`](../../doc/ku/process/pid/enum.Pid.html#variant.Id).

Системные вызовы, которые требуется реализовать в этой задаче ---
[`map()`](../../doc/kernel/process/syscall/fn.map.html),
[`unmap()`](../../doc/kernel/process/syscall/fn.unmap.html) и
[`copy_mapping()`](../../doc/kernel/process/syscall/fn.copy_mapping.html), ---
принимают на вход область памяти, которая должна быть выровнена на границы страниц.
В случае, когда либо её адрес либо её размер не выровнен, системные вызовы должны возвращать ошибку
[`Error::WrongAlignment`](../../doc/ku/error/enum.Error.html#variant.WrongAlignment).


### Валидация аргументов

Реализуйте функции валидации аргументов системных вызовов.


#### [`kernel::process::syscall::check_block()`](../../doc/kernel/process/syscall/fn.check_block.html)

```rust
fn check_block(
    address: usize,
    size: usize,
) -> Result<Block<Page>>
```

Проверяет, что `address` и `size` задают корректно выровненный диапазон страниц, целиком лежащий внутри одной из
[двух непрерывных половин](../../lab/book/2-mm-1-types.html#%D0%94%D0%B2%D0%B5-%D0%BF%D0%BE%D0%BB%D0%BE%D0%B2%D0%B8%D0%BD%D1%8B-%D0%B2%D0%B8%D1%80%D1%82%D1%83%D0%B0%D0%BB%D1%8C%D0%BD%D0%BE%D0%B3%D0%BE-%D0%B0%D0%B4%D1%80%D0%B5%D1%81%D0%BD%D0%BE%D0%B3%D0%BE-%D0%BF%D1%80%D0%BE%D1%81%D1%82%D1%80%D0%B0%D0%BD%D1%81%D1%82%D0%B2%D0%B0)
адресного пространства.


#### [`kernel::process::syscall::check_page_flags()`](../../doc/kernel/process/syscall/fn.check_page_flags.html)

```rust
fn check_page_flags(flags: usize) -> Result<PageTableFlags>
```

Проверяет, что `flags` задаёт валидный набор флагов отображения страниц.
И что во `flags` включён флаг разрешения доступа для пространства пользователя.
в противном случае возвращает ошибку
[`Error::PermissionDenied`](../../doc/kernel/error/enum.Error.html#variant.PermissionDenied).


#### [`kernel::process::syscall::check_frame()`](../../doc/kernel/process/syscall/fn.check_frame.html)

```rust
fn check_frame(
    process: &mut SpinlockGuard<Process>,
    page: Page,
    flags: PageTableFlags,
) -> Result<Frame>
```

Проверяет, что заданная виртуальная страница `page` отображена
в адресное пространство процесса `process` с корректно заданными флагами `flags` и
возвращает физический фрейм, в который она отображена.


#### [`kernel::process::syscall::lock_set::LockSet::new()`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#method.new)

```rust
fn new(
    src: SpinlockGuard<Process>,
    dst_pid: usize,
    process_set: ProcessSet,
) -> Result<LockSet<'_>>
```

Проверяет, что процесс `src` имеет право модифицировать целевой процесс, заданный своим идентификатором `dst_pid`.
Модифицировать можно либо самого себя, задавая
[`Pid::Current`](../../doc/ku/process/pid/enum.Pid.html#variant.Current)
или явно собственный идентификатор
[`Pid::Id`](../../doc/ku/process/pid/enum.Pid.html#variant.Id).
Либо свой непосредственно дочерний процесс, задавая его идентификатор.
Аргумент `process_set` типа
[`kernel::process::syscall::lock_set::ProcessSet`](../../doc/kernel/process/syscall/lock_set/enum.ProcessSet.html)
указывает, будет ли вызывающий код в дальнейшем пользоваться

- только целевым процессом --- [`ProcessSet::Dst`](../../doc/kernel/process/syscall/lock_set/enum.ProcessSet.html#variant.Dst),
- или же обоими процессами --- [`ProcessSet::SrcDst`](../../doc/kernel/process/syscall/lock_set/enum.ProcessSet.html#variant.SrcDst).

Функция возвращает один из вариантов
[`kernel::process::syscall::lock_set::LockSet`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html):

- [`LockSet::Different`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#variant.Different) с двумя блокировками на исходный [`LockSet::Different::src`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#variant.Different.field.src) и целевой [`LockSet::Different::dst`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#variant.Different.field.dst) процессы, когда они разные.
- [`LockSet::Same`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#variant.Same) с блокировкой [`LockSet::Same::src_dst`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#variant.Same.field.src_dst), когда вызывающий код запросил обе блокировки, но целевой процесс совпадает с исходным.
- [`LockSet::Dst`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#variant.Dst) с блокировкой [`LockSet::Dst::dst`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#variant.Dst.field.dst) на целевой процесс, когда вызывающему коду нужна только она.

В реализации этой функции легко посадить ошибку, которая может привести к
[взаимоблокировке](https://en.wikipedia.org/wiki/Deadlock).
Вспомните о правилах захвата нескольких блокировок, чтобы избежать этой неприятности.

Вам пригодится метод
[`Pid::from_usize()`](../../doc/ku/process/pid/enum.Pid.html#method.from_usize).

Пользоваться
[`LockSet`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html)
в системных вызовах можно так:

- [`kernel::process::syscall::lock_set::lock_src_dst(src, dst_pid)`](../../doc/kernel/process/syscall/lock_set/fn.lock_src_dst.html) вернёт [`LockSet`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html) с блокировками и на `src` и на `dst`.
- [`kernel::process::syscall::lock_set::lock_dst(src, dst_pid)`](../../doc/kernel/process/syscall/lock_set/fn.lock_dst.html) вернёт [`LockSet`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html) с блокировкой только на `dst`.
- [`LockSet::src()`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#method.src) и [`LockSet::src_mut()`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#method.src_mut) дают возможность использовать заблокированный исходный процесс иммутабельно и мутабельно соответственно, если [`LockSet`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html) создавался с захватом обоих блокировок. Если же захватывалась блокировка только на целевой процесс, эта функция запаникует.
- [`LockSet::dst()`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#method.dst) и [`LockSet::dst_mut()`](../../doc/kernel/process/syscall/lock_set/enum.LockSet.html#method.dst_mut) дают возможность использовать заблокированный целевой процесс иммутабельно и мутабельно соответственно.


### Системные вызовы для работы с памятью

Используя написанные вспомогательные функции, реализуйте следующие системные вызовы.


#### [`kernel::process::syscall::map()`](../../doc/kernel/process/syscall/fn.map.html)

```rust
fn map(
    process: SpinlockGuard<Process>,
    dst_pid: usize,
    dst_address: usize,
    dst_size: usize,
    flags: usize,
) -> Result<usize>
```

Отображает в памяти процесса, заданного `dst_pid`, блок страниц размера `dst_size` байт начиная с виртуального адреса `dst_address` с флагами доступа `flags`.
Если `dst_address` равен нулю, ядро само выбирает свободный участок адресного пространства размера `dst_size`.
Выбранный виртуальный адрес возвращается как результирующее значение системного вызова.
Размер при этом не возвращается, так как он должен быть равен аргументу `dst_address`.

Обратите внимание, что выбранный ядром виртуальный адрес должен быть корректно доставлен в
функцию пользователя, которая вызвала
[`lib::syscall::map()`](../../doc/lib/syscall/fn.map.html).
Референсная реализация этой функции в файле
[`user/lib/src/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/lib/src/syscall.rs)
предполагает, что ваша реализация функции
[`lib::syscall::syscall()`](../../doc/lib/syscall/fn.syscall.html)
помещает этот адрес в первый элемент возвращаемого кортежа.
Вы можете изменить это соглашение, но тогда поправьте и реализацию
[`lib::syscall::map()`](../../doc/lib/syscall/fn.map.html).

Также системный вызов
[`kernel::process::syscall::map()`](../../doc/kernel/process/syscall/fn.map.html)
должен поддерживать отсутствие флага
[`PageTableFlags::PRESENT`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.PRESENT)
во входном аргументе `flags`.
В этом случае происходит выделение адресного пространства --- виртуальных страниц.
Но не происходит выделения физических фреймов и их отображения.
Такой режим используется для реализации
[`lib::allocator::map::MapAllocator::reserve()`](../../doc/lib/allocator/map/struct.MapAllocator.html#method.reserve).
Сама структура
[`lib::allocator::map::MapAllocator`](../../doc/lib/allocator/map/struct.MapAllocator.html),
код которой уже написан, реализует
[знакомый](../../lab/book/3-smp-1-memory-allocator-0-traits.html#%D0%A2%D0%B8%D0%BF%D0%B0%D0%B6-kuallocatorbigbigallocator)
вам типаж
[`ku::allocator::big::BigAllocator`](../../doc/ku/allocator/big/trait.BigAllocator.html)
через системные вызовы
[`map()`](../../doc/kernel/process/syscall/fn.map.html),
[`unmap()`](../../doc/kernel/process/syscall/fn.unmap.html) и
[`copy_mapping()`](../../doc/kernel/process/syscall/fn.copy_mapping.html).
И позволяет аллоцировать память уже в пространстве пользователя.


### [`kernel::process::syscall::unmap()`](../../doc/kernel/process/syscall/fn.unmap.html)

```rust
fn unmap(
    process: SpinlockGuard<Process>,
    dst_pid: usize,
    dst_address: usize,
    dst_size: usize,
) -> Result<usize>
```

Выполняет противоположную
[`kernel::process::syscall::map()`](../../doc/kernel/process/syscall/fn.map.html)
операцию --- удаляет заданный диапазон из виртуальной памяти целевого процесса.


### [`kernel::process::syscall::copy_mapping()`](../../doc/kernel/process/syscall/fn.copy_mapping.html)

```rust
fn copy_mapping(
    process: SpinlockGuard<Process>,
    dst_pid: usize,
    src_address: usize,
    dst_address: usize,
    dst_size: usize,
    flags: usize,
) -> Result<usize>
```

Создаёт копию отображения виртуальной памяти из вызывающего процесса в процесс, заданный `dst_pid`.
Исходный диапазон начинается с виртуального адреса `src_address`, целевой --- с виртуального адреса `dst_address`.
Размер диапазона --- `dst_size` байт.
Системный вызов должен отобразить целевой диапазон в целевой процесс с флагами `flags`.
Естественно,
[`kernel::process::syscall::copy_mapping()`](../../doc/kernel/process/syscall/fn.copy_mapping.html)
не должен допускать целевое отображение с более широким по логическим привилегиям набором флагов, чем исходное.
При этом флаги
[`PageTableFlags::WRITABLE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.WRITABLE) и
[`PageTableFlags::COPY_ON_WRITE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.COPY_ON_WRITE)
с точки зрения ядра имеют одинаковые логические привилегии, --- доступность страницы на запись.

После его выполнения у процессов появляется область
[разделяемой памяти](https://en.wikipedia.org/wiki/Shared_memory).

Делать этот системный вызов транзакционным не требуется, но вы можете попробовать.

> Под транзакционностью подразумевается что сначала выполняются проверки всех доступов и физических фреймов.
> И только если они прошли, начинается копирование отображения виртуальных страниц.
> То есть, либо будет скопировано отображение всех виртуальных страниц и вызов завершится успешно.
> Либо вызов вернёт ошибку, не скопировав ни одно из отображений.
> Такая реализация сложнее чем наивная, которая может скопировать часть отображений страниц,
> а после этого вернуть ошибку.
> И для неё придётся очень аккуратно работать с двумя блокировками на процессы и корректностью кода
> с точки зрения семантики владения.
>
> Чтобы реализовать транзакционность, придётся пожертвовать либо значительным дополнительным временем,
> либо дополнительным временем и значительной дополнительной памятью.
> Если вы пойдёте по пути использования дополнительной памяти, вам также придётся правильно
> ответить на вопрос в каком адресном пространстве её выделять.
> И в этом случае вам могут пригодиться:
>
> - Метод [`Vec::new_in()`](https://doc.rust-lang.org/nightly/alloc/vec/struct.Vec.html#method.new_in).
> - Вспомогательная функция [`kernel::process::syscall::check_frames(process, block, flags)`](../../doc/kernel/process/syscall/fn.check_frames.html). Она проверяет, что заданный блок виртуальных страниц `block` отображён в адресное пространство процесса `process` с корректно заданными флагами `flags`. И возвращает вектор физических фреймов, в которые отображены эти страницы.
> - Вспомогательная функция [`kernel::process::syscall::map_pages_to_frames(process, src_frames, dst_pages, flags)`](../../doc/kernel/process/syscall/fn.map_pages_to_frames.html). Она выполняет отображение `src_frames` в `dst_pages` с флагами `flags` в адресное пространство процесса `process`.

Для упрощения реализации предлагается возвращать ошибку
[`Error::InvalidArgument`](../../doc/kernel/error/enum.Error.html#variant.InvalidArgument)
если исходный блок пересекается с целевым блоком внутри одного адресного пространства,
то есть когда целевой процесс совпадает с процессом, выполняющим системный вызов.
Но при желании вы можете реализовать копирование отображения и в этом случае.
Это особенно интересно, если вы решили реализовывать транзакционный вариант.


### Проверьте себя

Теперь должны заработать тесты `map_syscall_group()` и `user_space_memory_allocator()` в файле
[`kernel/tests/5-um-2-memory-allocator.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/5-um-2-memory-allocator.rs):

```console
$ (cd kernel; cargo test --test 5-um-2-memory-allocator)
...
5_um_2_memory_allocator::map_syscall_group------------------
20:53:54 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:53:54 0 I duplicate; address_space = "process" @ 0p7E2_6000
20:53:54 0 I switch to; address_space = "process" @ 0p7E2_6000
20:53:54 0 D extend mapping; block = [0v1000_0000, 0v1000_DEEC), size 55.730 KiB; page_block = [0v1000_0000, 0v1000_E000), size 56.000 KiB
20:53:54 0 D elf loadable program header; file_block = [0v20_3000, 0v21_0EEC), size 55.730 KiB; memory_block = [0v1000_0000, 0v1000_DEEC), size 55.730 KiB
20:53:54 0 D extend mapping; block = [0v1000_E000, 0v1008_94EB), size 493.229 KiB; page_block = [0v1000_E000, 0v1008_A000), size 496.000 KiB
20:53:54 0 D elf loadable program header; file_block = [0v21_0EF0, 0v28_C4EB), size 493.495 KiB; memory_block = [0v1000_DEF0, 0v1008_94EB), size 493.495 KiB
20:53:54 0 D elf loadable program header; file_block = [0v28_C4F0, 0v28_C630), size 320 B; memory_block = [0v1008_94F0, 0v1008_9630), size 320 B
20:53:54 0 D extend mapping; block = [0v1008_A000, 0v1017_5410), size 941.016 KiB; page_block = [0v1008_A000, 0v1017_6000), size 944.000 KiB
20:53:54 0 D elf loadable program header; file_block = [0v28_C630, 0v37_83E0), size 943.422 KiB; memory_block = [0v1008_9630, 0v1017_5410), size 943.469 KiB
20:53:54 0 I switch to; address_space = "base" @ 0p1000
20:53:54 0 I loaded ELF file; context = { rip: 0v1003_2EA0, rsp: 0v7F7F_FFFF_F000 }; file_size = 8.782 MiB; process = { pid: <current>, address_space: "process" @ 0p7E2_6000, { rip: 0v1003_2EA0, rsp: 0v7F7F_FFFF_F000 } }
20:53:54 0 I user process page table entry; entry_point = 0v1003_2EA0; frame = Frame(32220 @ 0p7DD_C000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:53:54 0 D process_frames = 437
20:53:54 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:54 0 D dropping; spinlock = kernel/tests/5-um-2-memory-allocator.rs:45:19; stats = Stats { failures: 0, locks: 54, unlocks: 54, waits: 0 }
20:53:54 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 24, unlocks: 24, waits: 0 }
20:53:54 0 I switch to; address_space = "base" @ 0p1000
20:53:54 0 I drop the current address space; address_space = "0:0" @ 0p7E2_6000; switch_to = "base" @ 0p1000
5_um_2_memory_allocator::map_syscall_group--------- [passed]

5_um_2_memory_allocator::user_space_memory_allocator--------
20:53:55.689 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:53:55.697 0 I duplicate; address_space = "process" @ 0p7E2_6000
20:53:55.701 0 I switch to; address_space = "process" @ 0p7E2_6000
20:53:55.709 0 D extend mapping; block = [0v1000_0000, 0v1000_DEEC), size 55.730 KiB; page_block = [0v1000_0000, 0v1000_E000), size 56.000 KiB
20:53:55.717 0 D elf loadable program header; file_block = [0v20_3000, 0v21_0EEC), size 55.730 KiB; memory_block = [0v1000_0000, 0v1000_DEEC), size 55.730 KiB
20:53:55.757 0 D extend mapping; block = [0v1000_E000, 0v1008_94EB), size 493.229 KiB; page_block = [0v1000_E000, 0v1008_A000), size 496.000 KiB
20:53:55.767 0 D elf loadable program header; file_block = [0v21_0EF0, 0v28_C4EB), size 493.495 KiB; memory_block = [0v1000_DEF0, 0v1008_94EB), size 493.495 KiB
20:53:55.785 0 D elf loadable program header; file_block = [0v28_C4F0, 0v28_C630), size 320 B; memory_block = [0v1008_94F0, 0v1008_9630), size 320 B
20:53:55.851 0 D extend mapping; block = [0v1008_A000, 0v1017_5410), size 941.016 KiB; page_block = [0v1008_A000, 0v1017_6000), size 944.000 KiB
20:53:55.861 0 D elf loadable program header; file_block = [0v28_C630, 0v37_83E0), size 943.422 KiB; memory_block = [0v1008_9630, 0v1017_5410), size 943.469 KiB
20:53:55.909 0 I switch to; address_space = "base" @ 0p1000
20:53:55.913 0 I loaded ELF file; context = { rip: 0v1003_2EA0, rsp: 0v7F7F_FFFF_F000 }; file_size = 8.782 MiB; process = { pid: <current>, address_space: "process" @ 0p7E2_6000, { rip: 0v1003_2EA0, rsp: 0v7F7F_FFFF_F000 } }
20:53:55.927 0 I allocate; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_6000, { rip: 0v1003_2EA0, rsp: 0v7F7F_FFFF_F000 } }; process_count = 1
20:53:55.937 0 I user process page table entry; entry_point = 0v1003_2EA0; frame = Frame(31912 @ 0p7CA_8000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:53:55.947 0 D process_frames = 437
20:53:55.951 0 I dequeue; pid = Some(0:0)
20:53:55.955 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:55.971 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1003_2EA0, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags: IF } }
20:53:55.981 0 I test_case = "basic"; pid = 0:0
20:53:55.993 0 D start_info = { allocations: 0 - 0 = 0, requested: 0 B - 0 B = 0 B, allocated: 0 B - 0 B = 0 B, pages: 0 - 0 = 0, loss: 0 B = 0.000% }; pid = 0:0
20:53:56.113 0 D leaving the user mode; pid = 0:0
20:53:56.117 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A2DC, ss:rsp: 0x001B:0v7F7F_FFFF_AB38, rflags: IF SF CF }
20:53:56.131 0 I dequeue; pid = Some(0:0)
20:53:56.133 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:56.147 0 D entering the user mode; pid = 0:0; registers = { rax: 0x88, rdi: 0x10121E50, rsi: 0x7F7FFFFFAD10, { mode: user, cs:rip: 0x0023:0v1005_A2DC, ss:rsp: 0x001B:0v7F7F_FFFF_AB38, rflags: IF SF CF } }
20:53:56.211 0 D leaving the user mode; pid = 0:0
20:53:56.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_F915, ss:rsp: 0x001B:0v7F7F_FFFF_93A8, rflags: IF }
20:53:56.225 0 I dequeue; pid = Some(0:0)
20:53:56.229 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:56.243 0 D entering the user mode; pid = 0:0; registers = { rax: 0x10094008, rdi: 0x10094008, rsi: 0x7FF, { mode: user, cs:rip: 0x0023:0v1003_F915, ss:rsp: 0x001B:0v7F7F_FFFF_93A8, rflags: IF } }
20:53:56.311 0 D leaving the user mode; pid = 0:0
20:53:56.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_A708, rflags: IF ZF PF }
20:53:56.329 0 I dequeue; pid = Some(0:0)
20:53:56.331 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:56.345 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFA960, rsi: 0x7F7FFFFFA8C0, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_A708, rflags: IF ZF PF } }
20:53:56.411 0 D leaving the user mode; pid = 0:0
20:53:56.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_B876, ss:rsp: 0x001B:0v7F7F_FFFF_A218, rflags: IF }
20:53:56.425 0 I dequeue; pid = Some(0:0)
20:53:56.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:56.443 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFA238, rdi: 0x7F7FFFFFA238, rsi: 0x7F7FFFFFA258, { mode: user, cs:rip: 0x0023:0v1004_B876, ss:rsp: 0x001B:0v7F7F_FFFF_A218, rflags: IF } }
20:53:56.511 0 D leaving the user mode; pid = 0:0
20:53:56.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_9358, rflags: IF ZF PF }
20:53:56.529 0 I dequeue; pid = Some(0:0)
20:53:56.531 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:56.545 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFF9548, rsi: 0x7F7FFFFF94A8, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_9358, rflags: IF ZF PF } }
20:53:56.041 0 D box_contents = 2; pid = 0:0
20:53:56.209 0 D allocator_info = { valid: true, total: { allocations: 1 - 0 = 1, requested: 4 B - 0 B = 4 B, allocated: 8 B - 0 B = 8 B, pages: 1 - 0 = 1, loss: 3.996 KiB = 99.902% }, fallback: { allocations: 0 - 0 = 0, requested: 0 B - 0 B = 0 B, allocated: 0 B - 0 B = 0 B, pages: 0 - 0 = 0, loss: 0 B = 0.000% }, size_8: { allocations: 1 - 0 = 1, requested: 4 B - 0 B = 4 B, allocated: 8 B - 0 B = 8 B, pages: 1 - 0 = 1, loss: 3.996 KiB = 99.902% } }; pid = 0:0
20:53:56.287 0 D info = { allocations: 1 - 0 = 1, requested: 4 B - 0 B = 4 B, allocated: 8 B - 0 B = 8 B, pages: 1 - 0 = 1, loss: 3.996 KiB = 99.902% }; pid = 0:0
20:53:56.289 0 D info_diff = { allocations: 1 - 0 = 1, requested: 4 B - 0 B = 4 B, allocated: 8 B - 0 B = 8 B, pages: 1 - 0 = 1, loss: 3.996 KiB = 99.902% }; pid = 0:0
20:53:56.499 0 D allocator_info = { valid: true, total: { allocations: 1 - 1 = 0, requested: 4 B - 4 B = 0 B, allocated: 8 B - 8 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, fallback: { allocations: 0 - 0 = 0, requested: 0 B - 0 B = 0 B, allocated: 0 B - 0 B = 0 B, pages: 0 - 0 = 0, loss: 0 B = 0.000% }, size_8: { allocations: 1 - 1 = 0, requested: 4 B - 4 B = 0 B, allocated: 8 B - 8 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% } }; pid = 0:0
20:53:56.579 0 D end_info = { allocations: 1 - 1 = 0, requested: 4 B - 4 B = 0 B, allocated: 8 B - 8 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }; pid = 0:0
20:53:56.581 0 D end_info_diff = { allocations: 1 - 1 = 0, requested: 4 B - 4 B = 0 B, allocated: 8 B - 8 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }; pid = 0:0
20:53:56.585 0 I test_case = "alignment"; pid = 0:0
20:53:56.585 0 D align = 1; pid = 0:0
20:53:57.011 0 D leaving the user mode; pid = 0:0
20:53:57.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_6E08, ss:rsp: 0x001B:0v7F7F_FFFF_D648, rflags: IF ZF PF }
20:53:57.029 0 I dequeue; pid = Some(0:0)
20:53:57.031 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:57.045 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFF00, rdi: 0x7F7FFFFFD688, rsi: 0x7F7FFFFFD6A8, { mode: user, cs:rip: 0x0023:0v1004_6E08, ss:rsp: 0x001B:0v7F7F_FFFF_D648, rflags: IF ZF PF } }
20:53:57.211 0 D leaving the user mode; pid = 0:0
20:53:57.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D808, rflags: IF ZF PF }
20:53:57.229 0 I dequeue; pid = Some(0:0)
20:53:57.231 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:57.245 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFDB60, rsi: 0x7F7FFFFFD998, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D808, rflags: IF ZF PF } }
20:53:58.111 0 D leaving the user mode; pid = 0:0
20:53:58.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A2C8, ss:rsp: 0x001B:0v7F7F_FFFF_DE58, rflags: IF ZF PF }
20:53:58.129 0 I dequeue; pid = Some(0:0)
20:53:58.131 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:58.145 0 D entering the user mode; pid = 0:0; registers = { rax: 0x75, rdi: 0x7F7FFFFFDF80, rsi: 0x7F7FFFFFDEF0, { mode: user, cs:rip: 0x0023:0v1005_A2C8, ss:rsp: 0x001B:0v7F7F_FFFF_DE58, rflags: IF ZF PF } }
20:53:58.211 0 D leaving the user mode; pid = 0:0
20:53:58.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D728, rflags: IF ZF PF }
20:53:58.229 0 I dequeue; pid = Some(0:0)
20:53:58.231 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:58.245 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFDA20, rsi: 0x7F7FFFFFD830, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D728, rflags: IF ZF PF } }
20:53:58.311 0 D leaving the user mode; pid = 0:0
20:53:58.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D978, rflags: IF ZF PF }
20:53:58.329 0 I dequeue; pid = Some(0:0)
20:53:58.331 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:58.345 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFDEF8, rsi: 0x7F7FFFFFDC48, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D978, rflags: IF ZF PF } }
20:53:58.411 0 D leaving the user mode; pid = 0:0
20:53:58.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_DE58, rflags: IF ZF PF }
20:53:58.429 0 I dequeue; pid = Some(0:0)
20:53:58.431 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:58.445 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFDF80, rsi: 0x7F7FFFFFDEF0, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_DE58, rflags: IF ZF PF } }
20:53:58.063 0 D align = 2; pid = 0:0
20:53:58.511 0 D leaving the user mode; pid = 0:0
20:53:58.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_1F48, ss:rsp: 0x001B:0v7F7F_FFFF_D970, rflags: IF ZF PF }
20:53:58.529 0 I dequeue; pid = Some(0:0)
20:53:58.531 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:58.545 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFDD18, rdi: 0x7F7FFFFFDD18, rsi: 0x7F7FFFFFDD38, { mode: user, cs:rip: 0x0023:0v1004_1F48, ss:rsp: 0x001B:0v7F7F_FFFF_D970, rflags: IF ZF PF } }
20:53:59.111 0 D leaving the user mode; pid = 0:0
20:53:59.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_DCE8, rflags: IF ZF PF }
20:53:59.127 0 I dequeue; pid = Some(0:0)
20:53:59.131 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:59.145 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFDE10, rsi: 0x7F7FFFFFDD80, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_DCE8, rflags: IF ZF PF } }
20:53:59.211 0 D leaving the user mode; pid = 0:0
20:53:59.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_8865, ss:rsp: 0x001B:0v7F7F_FFFF_D628, rflags: IF PF }
20:53:59.225 0 I dequeue; pid = Some(0:0)
20:53:59.229 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:59.243 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFD640, rsi: 0x7F7FFFFFDD48, { mode: user, cs:rip: 0x0023:0v1004_8865, ss:rsp: 0x001B:0v7F7F_FFFF_D628, rflags: IF PF } }
20:53:59.311 0 D leaving the user mode; pid = 0:0
20:53:59.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_5EE1, ss:rsp: 0x001B:0v7F7F_FFFF_DBC0, rflags: IF AF PF }
20:53:59.329 0 I dequeue; pid = Some(0:0)
20:53:59.331 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:59.345 0 D entering the user mode; pid = 0:0; registers = { rax: 0x10035EE1, rdi: 0x100B82D8, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1003_5EE1, ss:rsp: 0x001B:0v7F7F_FFFF_DBC0, rflags: IF AF PF } }
20:53:59.411 0 D leaving the user mode; pid = 0:0
20:53:59.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D808, rflags: IF ZF PF }
20:53:59.429 0 I dequeue; pid = Some(0:0)
20:53:59.431 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:53:59.445 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFDD88, rsi: 0x7F7FFFFFDAD8, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D808, rflags: IF ZF PF } }
20:53:59.085 0 D align = 4; pid = 0:0
20:54:00.111 0 D leaving the user mode; pid = 0:0
20:54:00.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D6D8, rflags: IF }
20:54:00.125 0 I dequeue; pid = Some(0:0)
20:54:00.129 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:00.143 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x7F7FFFFFD6C0, rsi: 0x7F7FFFFFDD48, { mode: user, cs:rip: 0x0023:0v1004_C0F5, ss:rsp: 0x001B:0v7F7F_FFFF_D6D8, rflags: IF } }
20:54:00.211 0 D leaving the user mode; pid = 0:0
20:54:00.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_8951, ss:rsp: 0x001B:0v7F7F_FFFF_D688, rflags: IF PF }
20:54:00.225 0 I dequeue; pid = Some(0:0)
20:54:00.229 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:00.243 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFDD48, rdi: 0x7F7FFFFFD6B8, rsi: 0x7F7FFFFFDD48, { mode: user, cs:rip: 0x0023:0v1004_8951, ss:rsp: 0x001B:0v7F7F_FFFF_D688, rflags: IF PF } }
20:54:00.311 0 D leaving the user mode; pid = 0:0
20:54:00.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C0D5, ss:rsp: 0x001B:0v7F7F_FFFF_D6C8, rflags: IF PF }
20:54:00.325 0 I dequeue; pid = Some(0:0)
20:54:00.329 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:00.343 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFE178, rdi: 0x7F7FFFFFD6D0, rsi: 0x7F7FFFFFE178, { mode: user, cs:rip: 0x0023:0v1004_C0D5, ss:rsp: 0x001B:0v7F7F_FFFF_D6C8, rflags: IF PF } }
20:54:00.411 0 D leaving the user mode; pid = 0:0
20:54:00.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1001_81B0, ss:rsp: 0x001B:0v7F7F_FFFF_E2E0, rflags: IF PF }
20:54:00.425 0 I dequeue; pid = Some(0:0)
20:54:00.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:00.443 0 D entering the user mode; pid = 0:0; registers = { rax: 0x3D, rdi: 0x7F7FFFFFE518, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1001_81B0, ss:rsp: 0x001B:0v7F7F_FFFF_E2E0, rflags: IF PF } }
20:54:00.061 0 D align = 8; pid = 0:0
20:54:00.511 0 D leaving the user mode; pid = 0:0
20:54:00.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1002_CE50, ss:rsp: 0x001B:0v7F7F_FFFF_E030, rflags: IF }
20:54:00.525 0 I dequeue; pid = Some(0:0)
20:54:00.529 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:00.543 0 D entering the user mode; pid = 0:0; registers = { rax: 0x2, rdi: 0x7F7FFFFFE0B8, rsi: 0x7F7FF0000, { mode: user, cs:rip: 0x0023:0v1002_CE50, ss:rsp: 0x001B:0v7F7F_FFFF_E030, rflags: IF } }
20:54:01.111 0 D leaving the user mode; pid = 0:0
20:54:01.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_A6CB, ss:rsp: 0x001B:0v7F7F_FFFF_D918, rflags: IF }
20:54:01.125 0 I dequeue; pid = Some(0:0)
20:54:01.129 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:01.143 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFDA58, rdi: 0x7F7FFFFFDA58, rsi: 0x8, { mode: user, cs:rip: 0x0023:0v1004_A6CB, ss:rsp: 0x001B:0v7F7F_FFFF_D918, rflags: IF } }
20:54:01.211 0 D leaving the user mode; pid = 0:0
20:54:01.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_81F2, ss:rsp: 0x001B:0v7F7F_FFFF_E0E8, rflags: IF ZF PF }
20:54:01.229 0 I dequeue; pid = Some(0:0)
20:54:01.231 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:01.245 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x10, rsi: 0x8, { mode: user, cs:rip: 0x0023:0v1003_81F2, ss:rsp: 0x001B:0v7F7F_FFFF_E0E8, rflags: IF ZF PF } }
20:54:01.069 0 D align = 16; pid = 0:0
20:54:01.311 0 D leaving the user mode; pid = 0:0
20:54:01.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_8865, ss:rsp: 0x001B:0v7F7F_FFFF_D818, rflags: IF }
20:54:01.325 0 I dequeue; pid = Some(0:0)
20:54:01.329 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:01.343 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFD830, rsi: 0x7F7FFFFFDEB8, { mode: user, cs:rip: 0x0023:0v1004_8865, ss:rsp: 0x001B:0v7F7F_FFFF_D818, rflags: IF } }
20:54:01.411 0 D leaving the user mode; pid = 0:0
20:54:01.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_E3B0, ss:rsp: 0x001B:0v7F7F_FFFF_D910, rflags: IF PF }
20:54:01.425 0 I dequeue; pid = Some(0:0)
20:54:01.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:01.443 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFDC20, rdi: 0x7F7FFFFFDBD0, rsi: 0x7F7FFFFFDBD0, { mode: user, cs:rip: 0x0023:0v1003_E3B0, ss:rsp: 0x001B:0v7F7F_FFFF_D910, rflags: IF PF } }
20:54:02.211 0 D leaving the user mode; pid = 0:0
20:54:02.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D978, rflags: IF ZF PF }
20:54:02.229 0 I dequeue; pid = Some(0:0)
20:54:02.231 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:02.245 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFDB08, rsi: 0x7F7FFFFFDAE0, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D978, rflags: IF ZF PF } }
20:54:02.311 0 D leaving the user mode; pid = 0:0
20:54:02.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_7000, ss:rsp: 0x001B:0v7F7F_FFFF_DF90, rflags: IF ZF PF }
20:54:02.329 0 I dequeue; pid = Some(0:0)
20:54:02.331 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:02.345 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFAD101, rdi: 0x100B7D00, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1005_7000, ss:rsp: 0x001B:0v7F7F_FFFF_DF90, rflags: IF ZF PF } }
20:54:02.131 0 D align = 32; pid = 0:0
20:54:02.411 0 D leaving the user mode; pid = 0:0
20:54:02.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D978, rflags: IF ZF PF }
20:54:02.429 0 I dequeue; pid = Some(0:0)
20:54:02.431 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:02.445 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFDBD0, rsi: 0x7F7FFFFFDB30, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D978, rflags: IF ZF PF } }
20:54:02.611 0 D leaving the user mode; pid = 0:0
20:54:02.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_9E41, ss:rsp: 0x001B:0v7F7F_FFFF_DF88, rflags: IF PF }
20:54:02.625 0 I dequeue; pid = Some(0:0)
20:54:02.629 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:02.643 0 D entering the user mode; pid = 0:0; registers = { rax: 0x21717C5, rdi: 0x10174080, rsi: 0x1FDF, { mode: user, cs:rip: 0x0023:0v1003_9E41, ss:rsp: 0x001B:0v7F7F_FFFF_DF88, rflags: IF PF } }
20:54:03.411 0 D leaving the user mode; pid = 0:0
20:54:03.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_EE20, ss:rsp: 0x001B:0v7F7F_FFFF_D520, rflags: IF PF }
20:54:03.425 0 I dequeue; pid = Some(0:0)
20:54:03.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:03.443 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFE008, rdi: 0x7F7FFFFFE008, rsi: 0x10, { mode: user, cs:rip: 0x0023:0v1004_EE20, ss:rsp: 0x001B:0v7F7F_FFFF_D520, rflags: IF PF } }
20:54:03.511 0 D leaving the user mode; pid = 0:0
20:54:03.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_79F0, ss:rsp: 0x001B:0v7F7F_FFFF_E080, rflags: IF PF }
20:54:03.525 0 I dequeue; pid = Some(0:0)
20:54:03.529 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:03.543 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFBD100, rdi: 0x100B6448, rsi: 0x7F7FFFFBD040, { mode: user, cs:rip: 0x0023:0v1004_79F0, ss:rsp: 0x001B:0v7F7F_FFFF_E080, rflags: IF PF } }
20:54:03.611 0 D leaving the user mode; pid = 0:0
20:54:03.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_1552, ss:rsp: 0x001B:0v7F7F_FFFF_E0B8, rflags: IF ZF PF }
20:54:03.629 0 I dequeue; pid = Some(0:0)
20:54:03.631 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:03.645 0 D entering the user mode; pid = 0:0; registers = { rax: 0x400, rdi: 0x3F9, rsi: 0x40, { mode: user, cs:rip: 0x0023:0v1003_1552, ss:rsp: 0x001B:0v7F7F_FFFF_E0B8, rflags: IF ZF PF } }
20:54:03.351 0 D align = 64; pid = 0:0
20:54:04.811 0 D leaving the user mode; pid = 0:0
20:54:04.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1002_F39F, ss:rsp: 0x001B:0v7F7F_FFFF_DEC8, rflags: IF PF }
20:54:04.827 0 I dequeue; pid = Some(0:0)
20:54:04.829 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:04.843 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1008BED0, rdi: 0x1008BED0, rsi: 0x1008BED0, { mode: user, cs:rip: 0x0023:0v1002_F39F, ss:rsp: 0x001B:0v7F7F_FFFF_DEC8, rflags: IF PF } }
20:54:04.911 0 D leaving the user mode; pid = 0:0
20:54:04.915 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_5C0C, ss:rsp: 0x001B:0v7F7F_FFFF_D878, rflags: IF }
20:54:04.925 0 I dequeue; pid = Some(0:0)
20:54:04.929 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:04.943 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFDD80, rsi: 0x1008B088, { mode: user, cs:rip: 0x0023:0v1004_5C0C, ss:rsp: 0x001B:0v7F7F_FFFF_D878, rflags: IF } }
20:54:05.011 0 D leaving the user mode; pid = 0:0
20:54:05.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_6CF6, ss:rsp: 0x001B:0v7F7F_FFFF_D7B8, rflags: IF }
20:54:05.025 0 I dequeue; pid = Some(0:0)
20:54:05.029 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:05.043 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFD7F8, rdi: 0x7F7FFFFFD7F8, rsi: 0x7F7FFFFFD818, { mode: user, cs:rip: 0x0023:0v1004_6CF6, ss:rsp: 0x001B:0v7F7F_FFFF_D7B8, rflags: IF } }
20:54:05.111 0 D leaving the user mode; pid = 0:0
20:54:05.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_B100, ss:rsp: 0x001B:0v7F7F_FFFF_D770, rflags: IF PF }
20:54:05.125 0 I dequeue; pid = Some(0:0)
20:54:05.129 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:05.143 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFD7B0, rsi: 0x7F7FFFFFDEB8, { mode: user, cs:rip: 0x0023:0v1004_B100, ss:rsp: 0x001B:0v7F7F_FFFF_D770, rflags: IF PF } }
20:54:04.809 0 D align = 128; pid = 0:0
20:54:06.711 0 D leaving the user mode; pid = 0:0
20:54:06.715 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D7E8, rflags: IF ZF PF }
20:54:06.729 0 I dequeue; pid = Some(0:0)
20:54:06.731 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:06.745 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFD9C0, rsi: 0x7F7FFFFFDA10, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D7E8, rflags: IF ZF PF } }
20:54:06.811 0 D leaving the user mode; pid = 0:0
20:54:06.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1007_6B30, ss:rsp: 0x001B:0v7F7F_FFFF_E1E0, rflags: IF }
20:54:06.825 0 I dequeue; pid = Some(0:0)
20:54:06.829 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:06.843 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFE518, rdi: 0x7F7FFFFFE518, rsi: 0xB5, { mode: user, cs:rip: 0x0023:0v1007_6B30, ss:rsp: 0x001B:0v7F7F_FFFF_E1E0, rflags: IF } }
20:54:06.649 0 D align = 256; pid = 0:0
20:54:06.911 0 D leaving the user mode; pid = 0:0
20:54:06.915 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1002_E8E5, ss:rsp: 0x001B:0v7F7F_FFFF_DCF8, rflags: IF }
20:54:06.925 0 I dequeue; pid = Some(0:0)
20:54:06.929 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:06.943 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1002E8E5, rdi: 0x100C0100, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1002_E8E5, ss:rsp: 0x001B:0v7F7F_FFFF_DCF8, rflags: IF } }
20:54:07.611 0 D leaving the user mode; pid = 0:0
20:54:07.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_715C, ss:rsp: 0x001B:0v7F7F_FFFF_D7A8, rflags: IF }
20:54:07.625 0 I dequeue; pid = Some(0:0)
20:54:07.629 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:07.643 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFD818, rdi: 0x7F7FFFFFD818, rsi: 0x7F7FFFFFDD49, { mode: user, cs:rip: 0x0023:0v1004_715C, ss:rsp: 0x001B:0v7F7F_FFFF_D7A8, rflags: IF } }
20:54:08.611 0 D leaving the user mode; pid = 0:0
20:54:08.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_B2E0, ss:rsp: 0x001B:0v7F7F_FFFF_E200, rflags: IF }
20:54:08.625 0 I dequeue; pid = Some(0:0)
20:54:08.629 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:08.643 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFF9B000, rdi: 0x7F7FFFF9B000, rsi: 0x6B, { mode: user, cs:rip: 0x0023:0v1005_B2E0, ss:rsp: 0x001B:0v7F7F_FFFF_E200, rflags: IF } }
20:54:08.711 0 D leaving the user mode; pid = 0:0
20:54:08.715 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_A310, ss:rsp: 0x001B:0v7F7F_FFFF_D6D8, rflags: IF }
20:54:08.725 0 I dequeue; pid = Some(0:0)
20:54:08.729 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:08.743 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFD818, rdi: 0x7F7FFFFFD818, rsi: 0x7F7FFFFFD000, { mode: user, cs:rip: 0x0023:0v1003_A310, ss:rsp: 0x001B:0v7F7F_FFFF_D6D8, rflags: IF } }
20:54:08.511 0 D align = 512; pid = 0:0
20:54:08.811 0 D leaving the user mode; pid = 0:0
20:54:08.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C090, ss:rsp: 0x001B:0v7F7F_FFFF_D860, rflags: IF ZF PF }
20:54:08.829 0 I dequeue; pid = Some(0:0)
20:54:08.831 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:08.845 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFDEB8, rdi: 0x7F7FFFFFD928, rsi: 0x7F7FFFFFDEB8, { mode: user, cs:rip: 0x0023:0v1004_C090, ss:rsp: 0x001B:0v7F7F_FFFF_D860, rflags: IF ZF PF } }
20:54:10.411 0 D leaving the user mode; pid = 0:0
20:54:10.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A2A0, ss:rsp: 0x001B:0v7F7F_FFFF_DD50, rflags: IF PF }
20:54:10.425 0 I dequeue; pid = Some(0:0)
20:54:10.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:10.443 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFDD80, rdi: 0x7F7FFFFFDE10, rsi: 0x7F7FFFFFDD80, { mode: user, cs:rip: 0x0023:0v1005_A2A0, ss:rsp: 0x001B:0v7F7F_FFFF_DD50, rflags: IF PF } }
20:54:10.511 0 D leaving the user mode; pid = 0:0
20:54:10.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_5F18, ss:rsp: 0x001B:0v7F7F_FFFF_DBE0, rflags: IF }
20:54:10.525 0 I dequeue; pid = Some(0:0)
20:54:10.529 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:10.543 0 D entering the user mode; pid = 0:0; registers = { rax: 0x19B9, rdi: 0x100C00D8, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1003_5F18, ss:rsp: 0x001B:0v7F7F_FFFF_DBE0, rflags: IF } }
20:54:10.611 0 D leaving the user mode; pid = 0:0
20:54:10.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A2DC, ss:rsp: 0x001B:0v7F7F_FFFF_D7E8, rflags: IF SF CF }
20:54:10.629 0 I dequeue; pid = Some(0:0)
20:54:10.631 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:10.645 0 D entering the user mode; pid = 0:0; registers = { rax: 0x50, rdi: 0x7F7FFFFFD9C0, rsi: 0x7F7FFFFFDA10, { mode: user, cs:rip: 0x0023:0v1005_A2DC, ss:rsp: 0x001B:0v7F7F_FFFF_D7E8, rflags: IF SF CF } }
20:54:10.333 0 D align = 1024; pid = 0:0
20:54:11.111 0 D leaving the user mode; pid = 0:0
20:54:11.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1002_CA45, ss:rsp: 0x001B:0v7F7F_FFFF_DA98, rflags: IF PF }
20:54:11.125 0 I dequeue; pid = Some(0:0)
20:54:11.129 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:11.143 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x0, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1002_CA45, ss:rsp: 0x001B:0v7F7F_FFFF_DA98, rflags: IF PF } }
20:54:12.211 0 D leaving the user mode; pid = 0:0
20:54:12.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_E1AD, ss:rsp: 0x001B:0v7F7F_FFFF_DE88, rflags: IF PF }
20:54:12.225 0 I dequeue; pid = Some(0:0)
20:54:12.229 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:12.243 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFF4D000, rdi: 0x100CC048, rsi: 0x8, { mode: user, cs:rip: 0x0023:0v1004_E1AD, ss:rsp: 0x001B:0v7F7F_FFFF_DE88, rflags: IF PF } }
20:54:12.311 0 D leaving the user mode; pid = 0:0
20:54:12.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_7A34, ss:rsp: 0x001B:0v7F7F_FFFF_DE48, rflags: IF }
20:54:12.325 0 I dequeue; pid = Some(0:0)
20:54:12.329 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:12.343 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x100CC048, rsi: 0x8, { mode: user, cs:rip: 0x0023:0v1004_7A34, ss:rsp: 0x001B:0v7F7F_FFFF_DE48, rflags: IF } }
20:54:12.411 0 D leaving the user mode; pid = 0:0
20:54:12.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_6AF6, ss:rsp: 0x001B:0v7F7F_FFFF_E128, rflags: IF PF }
20:54:12.425 0 I dequeue; pid = Some(0:0)
20:54:12.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:12.443 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFF4D000, rdi: 0x100CC048, rsi: 0x8, { mode: user, cs:rip: 0x0023:0v1003_6AF6, ss:rsp: 0x001B:0v7F7F_FFFF_E128, rflags: IF PF } }
20:54:12.511 0 D leaving the user mode; pid = 0:0
20:54:12.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D978, rflags: IF ZF PF }
20:54:12.529 0 I dequeue; pid = Some(0:0)
20:54:12.531 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:12.545 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFDC48, rsi: 0x7F7FFFFFDA58, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_D978, rflags: IF ZF PF } }
20:54:12.187 0 D align = 2048; pid = 0:0
20:54:13.111 0 D leaving the user mode; pid = 0:0
20:54:13.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C1F9, ss:rsp: 0x001B:0v7F7F_FFFF_DFE8, rflags: IF PF }
20:54:13.125 0 I dequeue; pid = Some(0:0)
20:54:13.129 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:13.143 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFD6CC800, rsi: 0x7F7FFFFFE048, { mode: user, cs:rip: 0x0023:0v1004_C1F9, ss:rsp: 0x001B:0v7F7F_FFFF_DFE8, rflags: IF PF } }
20:54:14.345 0 D align = 4096; pid = 0:0
20:54:15.511 0 D leaving the user mode; pid = 0:0
20:54:15.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1007_063D, ss:rsp: 0x001B:0v7F7F_FFFF_E1C8, rflags: IF PF }
20:54:15.525 0 I dequeue; pid = Some(0:0)
20:54:15.529 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:15.543 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFD0C3000, rsi: 0x1000, { mode: user, cs:rip: 0x0023:0v1007_063D, ss:rsp: 0x001B:0v7F7F_FFFF_E1C8, rflags: IF PF } }
20:54:21.171 0 I test_case = "grow_and_shrink"; pid = 0:0
20:54:21.173 0 D start_info = { allocations: 30239 - 30239 = 0, requested: 81.509 MiB - 81.509 MiB = 0 B, allocated: 99.165 MiB - 99.165 MiB = 0 B, pages: 15047 - 14034 = 1013, loss: 3.957 MiB = 100.000% }; pid = 0:0
20:54:21.311 0 D leaving the user mode; pid = 0:0
20:54:21.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_B100, ss:rsp: 0x001B:0v7F7F_FFFF_B100, rflags: IF }
20:54:21.325 0 I dequeue; pid = Some(0:0)
20:54:21.329 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:21.343 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFC1A8, rdi: 0x7F7FFFFFB138, rsi: 0x7F7FFFFFC1B7, { mode: user, cs:rip: 0x0023:0v1004_B100, ss:rsp: 0x001B:0v7F7F_FFFF_B100, rflags: IF } }
20:54:21.411 0 D leaving the user mode; pid = 0:0
20:54:21.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A2C8, ss:rsp: 0x001B:0v7F7F_FFFF_B588, rflags: IF ZF PF }
20:54:21.429 0 I dequeue; pid = Some(0:0)
20:54:21.431 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:21.445 0 D entering the user mode; pid = 0:0; registers = { rax: 0x26, rdi: 0x7F7FFFFFB718, rsi: 0x7F7FFFFFB6F0, { mode: user, cs:rip: 0x0023:0v1005_A2C8, ss:rsp: 0x001B:0v7F7F_FFFF_B588, rflags: IF ZF PF } }
20:54:21.511 0 D leaving the user mode; pid = 0:0
20:54:21.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1007_829B, ss:rsp: 0x001B:0v7F7F_FFFF_92F8, rflags: IF PF }
20:54:21.525 0 I dequeue; pid = Some(0:0)
20:54:21.529 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:21.543 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF95A8, rdi: 0x7F7FFFFF95A8, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1007_829B, ss:rsp: 0x001B:0v7F7F_FFFF_92F8, rflags: IF PF } }
20:54:21.611 0 D leaving the user mode; pid = 0:0
20:54:21.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_4CF0, ss:rsp: 0x001B:0v7F7F_FFFF_8F18, rflags: IF ZF PF }
20:54:21.629 0 I dequeue; pid = Some(0:0)
20:54:21.631 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:21.645 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1400, rdi: 0x7F7FFFFF9020, rsi: 0x7F7FFFFFBB78, { mode: user, cs:rip: 0x0023:0v1004_4CF0, ss:rsp: 0x001B:0v7F7F_FFFF_8F18, rflags: IF ZF PF } }
20:54:21.711 0 D leaving the user mode; pid = 0:0
20:54:21.715 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_1B80, ss:rsp: 0x001B:0v7F7F_FFFF_7650, rflags: IF PF }
20:54:21.725 0 I dequeue; pid = Some(0:0)
20:54:21.729 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:21.743 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF7698, rdi: 0x7F7FFFFF7678, rsi: 0x7F7FFFFF7698, { mode: user, cs:rip: 0x0023:0v1004_1B80, ss:rsp: 0x001B:0v7F7F_FFFF_7650, rflags: IF PF } }
20:54:21.811 0 D leaving the user mode; pid = 0:0
20:54:21.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_7776, ss:rsp: 0x001B:0v7F7F_FFFF_8AD8, rflags: IF }
20:54:21.825 0 I dequeue; pid = Some(0:0)
20:54:21.829 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:21.843 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFD7000, rdi: 0x7F7FFFFF8B08, rsi: 0x1008C478, { mode: user, cs:rip: 0x0023:0v1004_7776, ss:rsp: 0x001B:0v7F7F_FFFF_8AD8, rflags: IF } }
20:54:21.259 0 D contents_sum = 75491328; push_sum = 75491328; pid = 0:0
20:54:21.881 0 I syscall = "sched_yield"; pid = 0:0
20:54:21.885 0 D leaving the user mode; pid = 0:0
20:54:21.887 0 I dequeue; pid = Some(0:0)
20:54:21.891 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:21.905 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFD7000, rdi: 0x7F7FFFFF8B08, rsi: 0x1008C478, { mode: user, cs:rip: 0x0023:0v1002_C7E8, ss:rsp: 0x001B:0v7F7F_FFFF_BC28, rflags: IF } }
20:54:22.011 0 D leaving the user mode; pid = 0:0
20:54:22.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_7808, rflags: IF ZF PF }
20:54:22.029 0 I dequeue; pid = Some(0:0)
20:54:22.031 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:22.045 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFF7BE8, rsi: 0x7F7FFFFF7890, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_7808, rflags: IF ZF PF } }
20:54:22.111 0 D leaving the user mode; pid = 0:0
20:54:22.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_7398, rflags: IF ZF PF }
20:54:22.127 0 I dequeue; pid = Some(0:0)
20:54:22.131 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:22.145 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFF7460, rsi: 0x7F7FFFFF7498, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_7398, rflags: IF ZF PF } }
20:54:22.211 0 D leaving the user mode; pid = 0:0
20:54:22.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_7281, ss:rsp: 0x001B:0v7F7F_FFFF_9C08, rflags: IF }
20:54:22.225 0 I dequeue; pid = Some(0:0)
20:54:22.229 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:22.243 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF9CB8, rdi: 0x7F7FFFFF9CB8, rsi: 0x7F7FFFFF9C40, { mode: user, cs:rip: 0x0023:0v1004_7281, ss:rsp: 0x001B:0v7F7F_FFFF_9C08, rflags: IF } }
20:54:22.273 0 I syscall = "sched_yield"; pid = 0:0
20:54:22.277 0 D leaving the user mode; pid = 0:0
20:54:22.279 0 I dequeue; pid = Some(0:0)
20:54:22.283 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:22.297 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF9CB8, rdi: 0x7F7FFFFF9CB8, rsi: 0x7F7FFFFF9C40, { mode: user, cs:rip: 0x0023:0v1002_C7E8, ss:rsp: 0x001B:0v7F7F_FFFF_BC28, rflags: IF } }
20:54:22.311 0 D leaving the user mode; pid = 0:0
20:54:22.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A2C8, ss:rsp: 0x001B:0v7F7F_FFFF_9958, rflags: IF ZF PF }
20:54:22.327 0 I dequeue; pid = Some(0:0)
20:54:22.331 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:22.345 0 D entering the user mode; pid = 0:0; registers = { rax: 0x26, rdi: 0x7F7FFFFF9BC8, rsi: 0x7F7FFFFF9B08, { mode: user, cs:rip: 0x0023:0v1005_A2C8, ss:rsp: 0x001B:0v7F7F_FFFF_9958, rflags: IF ZF PF } }
20:54:22.379 0 E failed to log an event; error = RingBuffer(Overflow { capacity: 16380, len: 16380, exceeding_object_len: 1 }); timestamp = 2023-11-14 20:54:21.479905271 UTC; metadata = Metadata { name: "event user/lib/src/allocator/mod.rs:34", target: "lib::allocator", level: Level(Debug), module_path: "lib::allocator", location: user/lib/src/allocator/mod.rs:34, fields: {allocator_info}, callsite: Identifier(0x100b4080), kind: Kind(EVENT) }; message_prefix = "allocator_info = { valid: true, total: { allocations: 30252 - 30251 = 1, requested: 81.759 MiB - 81.634 MiB = 128.000 KiB, alloc"; pid = 0:0
20:54:22.395 0 D info = { allocations: 30252 - 30251 = 1, requested: 81.759 MiB - 81.634 MiB = 128.000 KiB, allocated: 99.414 MiB - 99.289 MiB = 128.000 KiB, pages: 15110 - 14065 = 1045, loss: 3.957 MiB = 96.938% }; pid = 0:0
20:54:22.401 0 E lost some log messages; recent = 1; total = 1; plan_b_failures = 0; plan_c_failures = 0; pid = 0:0
20:54:22.403 0 D info_diff = { allocations: 13 - 12 = 1, requested: 255.969 KiB - 127.969 KiB = 128.000 KiB, allocated: 255.969 KiB - 127.969 KiB = 128.000 KiB, pages: 63 - 31 = 32, loss: 0 B = 0.000% }; pid = 0:0
20:54:22.511 0 D leaving the user mode; pid = 0:0
20:54:22.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_BD28, rflags: IF ZF PF }
20:54:22.529 0 I dequeue; pid = Some(0:0)
20:54:22.531 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:22.545 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x10098F48, rsi: 0x7F7FFFFFBEF0, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_BD28, rflags: IF ZF PF } }
20:54:22.611 0 D leaving the user mode; pid = 0:0
20:54:22.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_8810, ss:rsp: 0x001B:0v7F7F_FFFF_B3E0, rflags: IF PF }
20:54:22.625 0 I dequeue; pid = Some(0:0)
20:54:22.629 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:22.643 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB570, rdi: 0x7F7FFFFFB570, rsi: 0x1002F3AE, { mode: user, cs:rip: 0x0023:0v1004_8810, ss:rsp: 0x001B:0v7F7F_FFFF_B3E0, rflags: IF PF } }
20:54:22.711 0 D leaving the user mode; pid = 0:0
20:54:22.715 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B568, rflags: IF ZF PF }
20:54:22.727 0 I dequeue; pid = Some(0:0)
20:54:22.731 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:22.745 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFB740, rsi: 0x7F7FFFFFB790, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B568, rflags: IF ZF PF } }
20:54:22.811 0 D leaving the user mode; pid = 0:0
20:54:22.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1006_98A3, ss:rsp: 0x001B:0v7F7F_FFFF_8AD8, rflags: IF }
20:54:22.825 0 I dequeue; pid = Some(0:0)
20:54:22.829 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:22.843 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x7F7FFFFF8CF7, rsi: 0x7F7FFFFD9793, { mode: user, cs:rip: 0x0023:0v1006_98A3, ss:rsp: 0x001B:0v7F7F_FFFF_8AD8, rflags: IF } }
20:54:22.911 0 D leaving the user mode; pid = 0:0
20:54:22.915 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1008_8400, ss:rsp: 0x001B:0v7F7F_FFFF_8F90, rflags: IF PF }
20:54:22.925 0 I dequeue; pid = Some(0:0)
20:54:22.929 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:22.943 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7FF0000000000000, rdi: 0x404C800000000000, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1008_8400, ss:rsp: 0x001B:0v7F7F_FFFF_8F90, rflags: IF PF } }
20:54:23.011 0 D leaving the user mode; pid = 0:0
20:54:23.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C4E0, ss:rsp: 0x001B:0v7F7F_FFFF_7650, rflags: IF PF }
20:54:23.025 0 I dequeue; pid = Some(0:0)
20:54:23.029 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:23.043 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x7F7FFFFF7698, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1004_C4E0, ss:rsp: 0x001B:0v7F7F_FFFF_7650, rflags: IF PF } }
20:54:22.489 0 D contents_sum = 75491328; pop_sum = 75491328; pid = 0:0
20:54:23.109 0 I syscall = "sched_yield"; pid = 0:0
20:54:23.113 0 D leaving the user mode; pid = 0:0
20:54:23.115 0 I dequeue; pid = Some(0:0)
20:54:23.119 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:23.133 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x7F7FFFFF7698, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1002_C7E8, ss:rsp: 0x001B:0v7F7F_FFFF_BC28, rflags: IF PF } }
20:54:23.211 0 D leaving the user mode; pid = 0:0
20:54:23.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_0EC5, ss:rsp: 0x001B:0v7F7F_FFFF_81B8, rflags: IF }
20:54:23.225 0 I dequeue; pid = Some(0:0)
20:54:23.229 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:23.243 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF81E8, rdi: 0x7F7FFFFF81E8, rsi: 0x7F7FFFFF8170, { mode: user, cs:rip: 0x0023:0v1005_0EC5, ss:rsp: 0x001B:0v7F7F_FFFF_81B8, rflags: IF } }
20:54:23.311 0 D leaving the user mode; pid = 0:0
20:54:23.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_8828, ss:rsp: 0x001B:0v7F7F_FFFF_8CD8, rflags: IF }
20:54:23.325 0 I dequeue; pid = Some(0:0)
20:54:23.329 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:23.343 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF8DF8, rdi: 0x7F7FFFFF8DF8, rsi: 0x7F7FFFFDEFFF, { mode: user, cs:rip: 0x0023:0v1004_8828, ss:rsp: 0x001B:0v7F7F_FFFF_8CD8, rflags: IF } }
20:54:23.411 0 D leaving the user mode; pid = 0:0
20:54:23.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_0A50, ss:rsp: 0x001B:0v7F7F_FFFF_7A90, rflags: IF PF }
20:54:23.425 0 I dequeue; pid = Some(0:0)
20:54:23.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:23.443 0 D entering the user mode; pid = 0:0; registers = { rax: 0x4000, rdi: 0x7F7FFFFF7AC0, rsi: 0x7F7FFFFF7B1E, { mode: user, cs:rip: 0x0023:0v1004_0A50, ss:rsp: 0x001B:0v7F7F_FFFF_7A90, rflags: IF PF } }
20:54:23.493 0 I syscall = "sched_yield"; pid = 0:0
20:54:23.497 0 D leaving the user mode; pid = 0:0
20:54:23.501 0 I dequeue; pid = Some(0:0)
20:54:23.505 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:23.519 0 D entering the user mode; pid = 0:0; registers = { rax: 0x4000, rdi: 0x7F7FFFFF7AC0, rsi: 0x7F7FFFFF7B1E, { mode: user, cs:rip: 0x0023:0v1002_C7E8, ss:rsp: 0x001B:0v7F7F_FFFF_BC28, rflags: IF PF } }
20:54:23.611 0 D leaving the user mode; pid = 0:0
20:54:23.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_EE20, ss:rsp: 0x001B:0v7F7F_FFFF_B100, rflags: IF }
20:54:23.625 0 I dequeue; pid = Some(0:0)
20:54:23.629 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:23.643 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFBEB8, rdi: 0x7F7FFFFFBEB8, rsi: 0x10, { mode: user, cs:rip: 0x0023:0v1004_EE20, ss:rsp: 0x001B:0v7F7F_FFFF_B100, rflags: IF } }
20:54:23.553 0 E failed to log an event; error = RingBuffer(Overflow { capacity: 16380, len: 16369, exceeding_object_len: 15 }); timestamp = 2023-11-14 20:54:22.755478341 UTC; metadata = Metadata { name: "event user/lib/src/allocator/mod.rs:34", target: "lib::allocator", level: Level(Debug), module_path: "lib::allocator", location: user/lib/src/allocator/mod.rs:34, fields: {allocator_info}, callsite: Identifier(0x100b4080), kind: Kind(EVENT) }; message_prefix = "allocator_info = { valid: true, total: { allocations: 30266 - 30266 = 0, requested: 81.884 MiB - 81.884 MiB = 0 B, allocated: 99"; pid = 0:0
20:54:23.557 0 D end_info = { allocations: 30266 - 30266 = 0, requested: 81.884 MiB - 81.884 MiB = 0 B, allocated: 99.539 MiB - 99.539 MiB = 0 B, pages: 15141 - 14128 = 1013, loss: 3.957 MiB = 100.000% }; pid = 0:0
20:54:23.561 0 E lost some log messages; recent = 1; total = 2; plan_b_failures = 0; plan_c_failures = 0; pid = 0:0
20:54:23.563 0 D end_info_diff = { allocations: 27 - 27 = 0, requested: 383.961 KiB - 383.961 KiB = 0 B, allocated: 383.961 KiB - 383.961 KiB = 0 B, pages: 94 - 94 = 0, loss: 0 B = 0.000% }; pid = 0:0
20:54:23.565 0 I test_case = "stress"; pid = 0:0
20:54:23.565 0 D start_info = { allocations: 30266 - 30266 = 0, requested: 81.884 MiB - 81.884 MiB = 0 B, allocated: 99.539 MiB - 99.539 MiB = 0 B, pages: 15141 - 14128 = 1013, loss: 3.957 MiB = 100.000% }; pid = 0:0
20:54:23.571 0 D vector_length = 0; pid = 0:0
20:54:23.769 0 D vector_length = 500; pid = 0:0
20:54:23.811 0 D leaving the user mode; pid = 0:0
20:54:23.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_1D20, ss:rsp: 0x001B:0v7F7F_FFFF_BBF0, rflags: IF PF }
20:54:23.825 0 I dequeue; pid = Some(0:0)
20:54:23.829 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:23.843 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFC4C85F8, rdi: 0x7F7FFFFFBC60, rsi: 0x196C, { mode: user, cs:rip: 0x0023:0v1003_1D20, ss:rsp: 0x001B:0v7F7F_FFFF_BBF0, rflags: IF PF } }
20:54:23.893 0 D vector_length = 1000; pid = 0:0
20:54:23.911 0 D leaving the user mode; pid = 0:0
20:54:23.915 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1002_A636, ss:rsp: 0x001B:0v7F7F_FFFF_BF28, rflags: IF PF }
20:54:23.925 0 I dequeue; pid = Some(0:0)
20:54:23.929 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:23.943 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFC4C4000, rdi: 0x7F7FFFFFBEF8, rsi: 0x1008A1B8, { mode: user, cs:rip: 0x0023:0v1002_A636, ss:rsp: 0x001B:0v7F7F_FFFF_BF28, rflags: IF PF } }
20:54:24.011 0 D leaving the user mode; pid = 0:0
20:54:24.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_D992, ss:rsp: 0x001B:0v7F7F_FFFF_B1A8, rflags: IF }
20:54:24.025 0 I dequeue; pid = Some(0:0)
20:54:24.029 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:24.043 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB2C8, rdi: 0x7F7FFFFFB100, rsi: 0x10019D8E, { mode: user, cs:rip: 0x0023:0v1004_D992, ss:rsp: 0x001B:0v7F7F_FFFF_B1A8, rflags: IF } }
20:54:24.059 0 D vector_length = 1500; pid = 0:0
20:54:24.111 0 D leaving the user mode; pid = 0:0
20:54:24.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_6C17, ss:rsp: 0x001B:0v7F7F_FFFF_AFD8, rflags: IF SF PF CF }
20:54:24.129 0 I dequeue; pid = Some(0:0)
20:54:24.131 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:24.145 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFBF18, rdi: 0x7F7FFFFFB078, rsi: 0x7F7FFFFFB098, { mode: user, cs:rip: 0x0023:0v1004_6C17, ss:rsp: 0x001B:0v7F7F_FFFF_AFD8, rflags: IF SF PF CF } }
20:54:24.181 0 D vector_length = 2000; pid = 0:0
20:54:24.211 0 D leaving the user mode; pid = 0:0
20:54:24.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A758, ss:rsp: 0x001B:0v7F7F_FFFF_BDA8, rflags: IF }
20:54:24.225 0 I dequeue; pid = Some(0:0)
20:54:24.229 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:24.243 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x101753E0, rsi: 0x8, { mode: user, cs:rip: 0x0023:0v1005_A758, ss:rsp: 0x001B:0v7F7F_FFFF_BDA8, rflags: IF } }
20:54:24.311 0 D leaving the user mode; pid = 0:0
20:54:24.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_B1E0, ss:rsp: 0x001B:0v7F7F_FFFF_B090, rflags: IF ZF PF }
20:54:24.329 0 I dequeue; pid = Some(0:0)
20:54:24.331 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:24.345 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFBBF8, rdi: 0x7F7FFFFFB228, rsi: 0x7F7FFFFFB120, { mode: user, cs:rip: 0x0023:0v1004_B1E0, ss:rsp: 0x001B:0v7F7F_FFFF_B090, rflags: IF ZF PF } }
20:54:24.307 0 D vector_length = 2500; pid = 0:0
20:54:24.411 0 D leaving the user mode; pid = 0:0
20:54:24.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_B6F2, ss:rsp: 0x001B:0v7F7F_FFFF_BB38, rflags: IF ZF PF }
20:54:24.429 0 I dequeue; pid = Some(0:0)
20:54:24.431 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:24.445 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFBBB0, rdi: 0x7F7FFFFFBBB0, rsi: 0x7F7FFC4B39B0, { mode: user, cs:rip: 0x0023:0v1004_B6F2, ss:rsp: 0x001B:0v7F7F_FFFF_BB38, rflags: IF ZF PF } }
20:54:24.473 0 D vector_length = 3000; pid = 0:0
20:54:24.511 0 D leaving the user mode; pid = 0:0
20:54:24.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C170, ss:rsp: 0x001B:0v7F7F_FFFF_B170, rflags: IF PF }
20:54:24.525 0 I dequeue; pid = Some(0:0)
20:54:24.529 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:24.543 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x7F7FFFFFB1E8, rsi: 0x7F7FFFFFB670, { mode: user, cs:rip: 0x0023:0v1004_C170, ss:rsp: 0x001B:0v7F7F_FFFF_B170, rflags: IF PF } }
20:54:24.591 0 D vector_length = 3500; pid = 0:0
20:54:24.611 0 D leaving the user mode; pid = 0:0
20:54:24.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C4E0, ss:rsp: 0x001B:0v7F7F_FFFF_B140, rflags: IF }
20:54:24.625 0 I dequeue; pid = Some(0:0)
20:54:24.629 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:24.643 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x7F7FFFFFB188, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1004_C4E0, ss:rsp: 0x001B:0v7F7F_FFFF_B140, rflags: IF } }
20:54:24.711 0 D leaving the user mode; pid = 0:0
20:54:24.715 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1002_E8E5, ss:rsp: 0x001B:0v7F7F_FFFF_B768, rflags: IF }
20:54:24.725 0 I dequeue; pid = Some(0:0)
20:54:24.729 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:24.743 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1002E8E5, rdi: 0x100B4280, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1002_E8E5, ss:rsp: 0x001B:0v7F7F_FFFF_B768, rflags: IF } }
20:54:24.709 0 D vector_length = 4000; pid = 0:0
20:54:24.811 0 D leaving the user mode; pid = 0:0
20:54:24.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B818, rflags: IF ZF PF }
20:54:24.829 0 I dequeue; pid = Some(0:0)
20:54:24.831 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:24.845 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x100B41D0, rsi: 0x7F7FFFFFB9F0, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B818, rflags: IF ZF PF } }
20:54:24.883 0 D vector_length = 4500; pid = 0:0
20:54:24.911 0 D leaving the user mode; pid = 0:0
20:54:24.915 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_AF50, ss:rsp: 0x001B:0v7F7F_FFFF_B9E0, rflags: IF PF }
20:54:24.925 0 I dequeue; pid = Some(0:0)
20:54:24.929 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:24.943 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x100B41C8, rsi: 0x8, { mode: user, cs:rip: 0x0023:0v1004_AF50, ss:rsp: 0x001B:0v7F7F_FFFF_B9E0, rflags: IF PF } }
20:54:25.011 0 D leaving the user mode; pid = 0:0
20:54:25.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1001_9E1C, ss:rsp: 0x001B:0v7F7F_FFFF_BE18, rflags: IF }
20:54:25.025 0 I dequeue; pid = Some(0:0)
20:54:25.029 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:25.043 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFC49EDD8, rdi: 0x100B4270, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1001_9E1C, ss:rsp: 0x001B:0v7F7F_FFFF_BE18, rflags: IF } }
20:54:25.003 0 D vector_length = 5000; pid = 0:0
20:54:25.111 0 D leaving the user mode; pid = 0:0
20:54:25.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A2C8, ss:rsp: 0x001B:0v7F7F_FFFF_B308, rflags: IF ZF PF }
20:54:25.127 0 I dequeue; pid = Some(0:0)
20:54:25.131 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:25.145 0 D entering the user mode; pid = 0:0; registers = { rax: 0x4, rdi: 0x7F7FFFFFB600, rsi: 0x7F7FFFFFB410, { mode: user, cs:rip: 0x0023:0v1005_A2C8, ss:rsp: 0x001B:0v7F7F_FFFF_B308, rflags: IF ZF PF } }
20:54:25.167 0 D vector_length = 5500; pid = 0:0
20:54:25.211 0 D leaving the user mode; pid = 0:0
20:54:25.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1001_080A, ss:rsp: 0x001B:0v7F7F_FFFF_BC38, rflags: IF PF }
20:54:25.225 0 I dequeue; pid = Some(0:0)
20:54:25.229 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:25.243 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFBC50, rdi: 0x7F7FFFFFBC50, rsi: 0x7F7FFFFB1040, { mode: user, cs:rip: 0x0023:0v1001_080A, ss:rsp: 0x001B:0v7F7F_FFFF_BC38, rflags: IF PF } }
20:54:25.311 0 D leaving the user mode; pid = 0:0
20:54:25.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C140, ss:rsp: 0x001B:0v7F7F_FFFF_B168, rflags: IF PF }
20:54:25.325 0 I dequeue; pid = Some(0:0)
20:54:25.329 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:25.343 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFBDB8, rdi: 0x7F7FFFFFB1E8, rsi: 0x7F7FFFFFB210, { mode: user, cs:rip: 0x0023:0v1004_C140, ss:rsp: 0x001B:0v7F7F_FFFF_B168, rflags: IF PF } }
20:54:25.291 0 D vector_length = 6000; pid = 0:0
20:54:25.411 0 D leaving the user mode; pid = 0:0
20:54:25.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B818, rflags: IF ZF PF }
20:54:25.427 0 I dequeue; pid = Some(0:0)
20:54:25.431 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:25.445 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x100B41D0, rsi: 0x7F7FFFFFB9F0, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B818, rflags: IF ZF PF } }
20:54:25.457 0 D vector_length = 6500; pid = 0:0
20:54:25.511 0 D leaving the user mode; pid = 0:0
20:54:25.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B3E8, rflags: IF ZF PF }
20:54:25.529 0 I dequeue; pid = Some(0:0)
20:54:25.533 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:25.545 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFB968, rsi: 0x7F7FFFFFB6B8, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B3E8, rflags: IF ZF PF } }
20:54:25.581 0 D vector_length = 7000; pid = 0:0
20:54:25.661 0 D vector_length = 7500; pid = 0:0
20:54:25.711 0 D leaving the user mode; pid = 0:0
20:54:25.715 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_88E8, ss:rsp: 0x001B:0v7F7F_FFFF_AEF8, rflags: IF }
20:54:25.725 0 I dequeue; pid = Some(0:0)
20:54:25.729 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:25.743 0 D entering the user mode; pid = 0:0; registers = { rax: 0x11, rdi: 0x7F7FFFFFB098, rsi: 0x7F7FFFFFBDC7, { mode: user, cs:rip: 0x0023:0v1004_88E8, ss:rsp: 0x001B:0v7F7F_FFFF_AEF8, rflags: IF } }
20:54:25.783 0 D vector_length = 8000; pid = 0:0
20:54:25.879 0 D vector_length = 8500; pid = 0:0
20:54:25.959 0 D vector_length = 9000; pid = 0:0
20:54:26.011 0 D leaving the user mode; pid = 0:0
20:54:26.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_7710, ss:rsp: 0x001B:0v7F7F_FFFF_B430, rflags: IF }
20:54:26.025 0 I dequeue; pid = Some(0:0)
20:54:26.029 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:26.043 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB, rdi: 0x7F7FFFFFB, rsi: 0x7F7FFFFFC, { mode: user, cs:rip: 0x0023:0v1004_7710, ss:rsp: 0x001B:0v7F7F_FFFF_B430, rflags: IF } }
20:54:26.111 0 D leaving the user mode; pid = 0:0
20:54:26.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_B0C8, ss:rsp: 0x001B:0v7F7F_FFFF_AF98, rflags: IF PF }
20:54:26.125 0 I dequeue; pid = Some(0:0)
20:54:26.129 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:26.143 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB038, rdi: 0x7F7FFFFFB038, rsi: 0x7F7FFFFFE6F8, { mode: user, cs:rip: 0x0023:0v1004_B0C8, ss:rsp: 0x001B:0v7F7F_FFFF_AF98, rflags: IF PF } }
20:54:26.083 0 D vector_length = 9500; pid = 0:0
20:54:26.211 0 D leaving the user mode; pid = 0:0
20:54:26.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B218, rflags: IF ZF PF }
20:54:26.229 0 I dequeue; pid = Some(0:0)
20:54:26.231 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:26.245 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFB510, rsi: 0x7F7FFFFFB320, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B218, rflags: IF ZF PF } }
20:54:26.311 0 D leaving the user mode; pid = 0:0
20:54:26.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B728, rflags: IF ZF PF }
20:54:26.329 0 I dequeue; pid = Some(0:0)
20:54:26.331 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:26.345 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x100FDB50, rsi: 0x7F7FFFFFB900, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B728, rflags: IF ZF PF } }
20:54:26.411 0 D leaving the user mode; pid = 0:0
20:54:26.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A2DC, ss:rsp: 0x001B:0v7F7F_FFFF_B2F8, rflags: IF SF AF CF }
20:54:26.427 0 I dequeue; pid = Some(0:0)
20:54:26.431 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:26.445 0 D entering the user mode; pid = 0:0; registers = { rax: 0x28, rdi: 0x7F7FFFFFB650, rsi: 0x7F7FFFFFB488, { mode: user, cs:rip: 0x0023:0v1005_A2DC, ss:rsp: 0x001B:0v7F7F_FFFF_B2F8, rflags: IF SF AF CF } }
20:54:26.511 0 D leaving the user mode; pid = 0:0
20:54:26.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C4E0, ss:rsp: 0x001B:0v7F7F_FFFF_9BC0, rflags: IF PF }
20:54:26.525 0 I dequeue; pid = Some(0:0)
20:54:26.529 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:26.543 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x7F7FFFFF9C28, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1004_C4E0, ss:rsp: 0x001B:0v7F7F_FFFF_9BC0, rflags: IF PF } }
20:54:26.611 0 D leaving the user mode; pid = 0:0
20:54:26.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_8198, rflags: IF ZF PF }
20:54:26.629 0 I dequeue; pid = Some(0:0)
20:54:26.631 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:26.645 0 D entering the user mode; pid = 0:0; registers = { rax: 0x200, rdi: 0x7F7FFFFF8258, rsi: 0x7F7FFFFF8B58, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_8198, rflags: IF ZF PF } }
20:54:26.711 0 D leaving the user mode; pid = 0:0
20:54:26.715 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_8810, ss:rsp: 0x001B:0v7F7F_FFFF_7940, rflags: IF PF }
20:54:26.725 0 I dequeue; pid = Some(0:0)
20:54:26.729 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:26.743 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF7990, rdi: 0x7F7FFFFF7990, rsi: 0x7F7FFFFD8122, { mode: user, cs:rip: 0x0023:0v1004_8810, ss:rsp: 0x001B:0v7F7F_FFFF_7940, rflags: IF PF } }
20:54:26.811 0 D leaving the user mode; pid = 0:0
20:54:26.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1007_0E10, ss:rsp: 0x001B:0v7F7F_FFFF_71A0, rflags: IF }
20:54:26.825 0 I dequeue; pid = Some(0:0)
20:54:26.829 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:26.843 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x2, rsi: 0x2, { mode: user, cs:rip: 0x0023:0v1007_0E10, ss:rsp: 0x001B:0v7F7F_FFFF_71A0, rflags: IF } }
20:54:26.209 0 D contents_sum = 333283335000; push_sum = 333283335000; pid = 0:0
20:54:26.891 0 I syscall = "sched_yield"; pid = 0:0
20:54:26.893 0 D leaving the user mode; pid = 0:0
20:54:26.897 0 I dequeue; pid = Some(0:0)
20:54:26.901 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:26.915 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x2, rsi: 0x2, { mode: user, cs:rip: 0x0023:0v1002_C7E8, ss:rsp: 0x001B:0v7F7F_FFFF_B998, rflags: IF } }
20:54:27.011 0 D leaving the user mode; pid = 0:0
20:54:27.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_7766, ss:rsp: 0x001B:0v7F7F_FFFF_8158, rflags: IF PF }
20:54:27.025 0 I dequeue; pid = Some(0:0)
20:54:27.029 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:27.043 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF8188, rdi: 0x7F7FFFFF7F50, rsi: 0x7F7FFFFDEFFF, { mode: user, cs:rip: 0x0023:0v1004_7766, ss:rsp: 0x001B:0v7F7F_FFFF_8158, rflags: IF PF } }
20:54:27.111 0 D leaving the user mode; pid = 0:0
20:54:27.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C0E0, ss:rsp: 0x001B:0v7F7F_FFFF_97A0, rflags: IF SF PF CF }
20:54:27.129 0 I dequeue; pid = Some(0:0)
20:54:27.131 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:27.145 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFD7000, rdi: 0x7F7FFFFD7000, rsi: 0x7F7FFFFDEFFF, { mode: user, cs:rip: 0x0023:0v1004_C0E0, ss:rsp: 0x001B:0v7F7F_FFFF_97A0, rflags: IF SF PF CF } }
20:54:27.211 0 D leaving the user mode; pid = 0:0
20:54:27.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_71FA, ss:rsp: 0x001B:0v7F7F_FFFF_7EA8, rflags: IF SF PF CF }
20:54:27.229 0 I dequeue; pid = Some(0:0)
20:54:27.231 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:27.245 0 D entering the user mode; pid = 0:0; registers = { rax: 0x65FB, rdi: 0x7F7FFFFF7F58, rsi: 0x7F7FFFFF7F70, { mode: user, cs:rip: 0x0023:0v1004_71FA, ss:rsp: 0x001B:0v7F7F_FFFF_7EA8, rflags: IF SF PF CF } }
20:54:27.281 0 I syscall = "sched_yield"; pid = 0:0
20:54:27.285 0 D leaving the user mode; pid = 0:0
20:54:27.289 0 I dequeue; pid = Some(0:0)
20:54:27.293 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:27.307 0 D entering the user mode; pid = 0:0; registers = { rax: 0x65FB, rdi: 0x7F7FFFFF7F58, rsi: 0x7F7FFFFF7F70, { mode: user, cs:rip: 0x0023:0v1002_C7E8, ss:rsp: 0x001B:0v7F7F_FFFF_B998, rflags: IF SF PF CF } }
20:54:27.411 0 D leaving the user mode; pid = 0:0
20:54:27.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_5990, ss:rsp: 0x001B:0v7F7F_FFFF_B440, rflags: IF }
20:54:27.425 0 I dequeue; pid = Some(0:0)
20:54:27.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:27.443 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB7A0, rdi: 0x7F7FFFFFB650, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1004_5990, ss:rsp: 0x001B:0v7F7F_FFFF_B440, rflags: IF } }
20:54:27.511 0 D leaving the user mode; pid = 0:0
20:54:27.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_EF0A, ss:rsp: 0x001B:0v7F7F_FFFF_B3A0, rflags: IF ZF PF }
20:54:27.529 0 I dequeue; pid = Some(0:0)
20:54:27.531 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:27.545 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFC000, rdi: 0x7F7FFFFFC, rsi: 0x1000, { mode: user, cs:rip: 0x0023:0v1004_EF0A, ss:rsp: 0x001B:0v7F7F_FFFF_B3A0, rflags: IF ZF PF } }
20:54:27.611 0 D leaving the user mode; pid = 0:0
20:54:27.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_1890, ss:rsp: 0x001B:0v7F7F_FFFF_B110, rflags: IF }
20:54:27.625 0 I dequeue; pid = Some(0:0)
20:54:27.629 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:27.643 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB220, rdi: 0x7F7FFFFFB200, rsi: 0x7F7FFFFFB220, { mode: user, cs:rip: 0x0023:0v1004_1890, ss:rsp: 0x001B:0v7F7F_FFFF_B110, rflags: IF } }
20:54:27.339 0 E failed to log an event; error = RingBuffer(Overflow { capacity: 16380, len: 16380, exceeding_object_len: 1 }); timestamp = 2023-11-14 20:54:26.492388589 UTC; metadata = Metadata { name: "event user/lib/src/allocator/mod.rs:34", target: "lib::allocator", level: Level(Debug), module_path: "lib::allocator", location: user/lib/src/allocator/mod.rs:34, fields: {allocator_info}, callsite: Identifier(0x100b4080), kind: Kind(EVENT) }; message_prefix = "allocator_info = { valid: true, total: { allocations: 40283 - 30278 = 10005, requested: 82.211 MiB - 82.009 MiB = 206.969 KiB, a"; pid = 0:0
20:54:27.345 0 D info = { allocations: 40283 - 30278 = 10005, requested: 82.211 MiB - 82.009 MiB = 206.969 KiB, allocated: 99.867 MiB - 99.664 MiB = 206.969 KiB, pages: 15223 - 14159 = 1064, loss: 3.954 MiB = 95.137% }; pid = 0:0
20:54:27.349 0 E lost some log messages; recent = 1; total = 3; plan_b_failures = 0; plan_c_failures = 0; pid = 0:0
20:54:27.351 0 D info_diff = { allocations: 10017 - 12 = 10005, requested: 334.938 KiB - 127.969 KiB = 206.969 KiB, allocated: 334.938 KiB - 127.969 KiB = 206.969 KiB, pages: 82 - 31 = 51, loss: 0 B = 0.000% }; pid = 0:0
20:54:27.455 0 D vector_length = 9500; pid = 0:0
20:54:27.557 0 D vector_length = 9000; pid = 0:0
20:54:27.659 0 D vector_length = 8500; pid = 0:0
20:54:27.811 0 D leaving the user mode; pid = 0:0
20:54:27.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_D79C, ss:rsp: 0x001B:0v7F7F_FFFF_B308, rflags: IF }
20:54:27.825 0 I dequeue; pid = Some(0:0)
20:54:27.829 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:27.843 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB350, rdi: 0x7F7FFFFFB350, rsi: 0x7F7FFFFFBF18, { mode: user, cs:rip: 0x0023:0v1004_D79C, ss:rsp: 0x001B:0v7F7F_FFFF_B308, rflags: IF } }
20:54:27.911 0 D leaving the user mode; pid = 0:0
20:54:27.915 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B548, rflags: IF ZF PF }
20:54:27.929 0 I dequeue; pid = Some(0:0)
20:54:27.931 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:27.945 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFB6D8, rsi: 0x7F7FFFFFB6B0, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B548, rflags: IF ZF PF } }
20:54:28.011 0 D leaving the user mode; pid = 0:0
20:54:28.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_0875, ss:rsp: 0x001B:0v7F7F_FFFF_B478, rflags: IF ZF PF }
20:54:28.029 0 I dequeue; pid = Some(0:0)
20:54:28.031 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:28.045 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFB4D0, rsi: 0x1008ECE0, { mode: user, cs:rip: 0x0023:0v1004_0875, ss:rsp: 0x001B:0v7F7F_FFFF_B478, rflags: IF ZF PF } }
20:54:28.111 0 D leaving the user mode; pid = 0:0
20:54:28.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_DA3F, ss:rsp: 0x001B:0v7F7F_FFFF_B118, rflags: IF }
20:54:28.125 0 I dequeue; pid = Some(0:0)
20:54:28.129 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:28.143 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB140, rdi: 0x7F7FFFFFB140, rsi: 0x7F7FFFFFB160, { mode: user, cs:rip: 0x0023:0v1004_DA3F, ss:rsp: 0x001B:0v7F7F_FFFF_B118, rflags: IF } }
20:54:28.211 0 D leaving the user mode; pid = 0:0
20:54:28.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B548, rflags: IF ZF PF }
20:54:28.227 0 I dequeue; pid = Some(0:0)
20:54:28.231 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:28.245 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFB818, rsi: 0x7F7FFFFFB628, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B548, rflags: IF ZF PF } }
20:54:28.311 0 D leaving the user mode; pid = 0:0
20:54:28.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_8810, ss:rsp: 0x001B:0v7F7F_FFFF_B1B0, rflags: IF PF }
20:54:28.327 0 I dequeue; pid = Some(0:0)
20:54:28.329 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:28.343 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB288, rdi: 0x7F7FFFFFB288, rsi: 0x7F7FFFFFBE38, { mode: user, cs:rip: 0x0023:0v1004_8810, ss:rsp: 0x001B:0v7F7F_FFFF_B1B0, rflags: IF PF } }
20:54:28.411 0 D leaving the user mode; pid = 0:0
20:54:28.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_D7E1, ss:rsp: 0x001B:0v7F7F_FFFF_B308, rflags: IF }
20:54:28.425 0 I dequeue; pid = Some(0:0)
20:54:28.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:28.443 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFB348, rsi: 0x7F7FFFFFB370, { mode: user, cs:rip: 0x0023:0v1004_D7E1, ss:rsp: 0x001B:0v7F7F_FFFF_B308, rflags: IF } }
20:54:28.511 0 D leaving the user mode; pid = 0:0
20:54:28.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_D9E9, ss:rsp: 0x001B:0v7F7F_FFFF_B118, rflags: IF }
20:54:28.525 0 I dequeue; pid = Some(0:0)
20:54:28.529 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:28.543 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB1A0, rdi: 0x7F7FFFFFB1A0, rsi: 0x7F7FFFFFBDD8, { mode: user, cs:rip: 0x0023:0v1004_D9E9, ss:rsp: 0x001B:0v7F7F_FFFF_B118, rflags: IF } }
20:54:27.791 0 D vector_length = 8000; pid = 0:0
20:54:27.891 0 D vector_length = 7500; pid = 0:0
20:54:27.993 0 D vector_length = 7000; pid = 0:0
20:54:28.097 0 D vector_length = 6500; pid = 0:0
20:54:28.197 0 D vector_length = 6000; pid = 0:0
20:54:28.299 0 D vector_length = 5500; pid = 0:0
20:54:28.399 0 D vector_length = 5000; pid = 0:0
20:54:28.499 0 D vector_length = 4500; pid = 0:0
20:54:28.711 0 D leaving the user mode; pid = 0:0
20:54:28.715 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B548, rflags: IF ZF PF }
20:54:28.729 0 I dequeue; pid = Some(0:0)
20:54:28.731 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:28.745 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFB818, rsi: 0x7F7FFFFFB628, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B548, rflags: IF ZF PF } }
20:54:28.811 0 D leaving the user mode; pid = 0:0
20:54:28.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_B840, ss:rsp: 0x001B:0v7F7F_FFFF_B110, rflags: IF ZF PF }
20:54:28.829 0 I dequeue; pid = Some(0:0)
20:54:28.831 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:28.845 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFB160, rsi: 0x7F7FFFFFBEF8, { mode: user, cs:rip: 0x0023:0v1004_B840, ss:rsp: 0x001B:0v7F7F_FFFF_B110, rflags: IF ZF PF } }
20:54:28.911 0 D leaving the user mode; pid = 0:0
20:54:28.915 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_C100, ss:rsp: 0x001B:0v7F7F_FFFF_B300, rflags: IF }
20:54:28.925 0 I dequeue; pid = Some(0:0)
20:54:28.929 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:28.943 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFBD48, rdi: 0x7F7FFFFFB350, rsi: 0x10008B02, { mode: user, cs:rip: 0x0023:0v1004_C100, ss:rsp: 0x001B:0v7F7F_FFFF_B300, rflags: IF } }
20:54:28.631 0 D vector_length = 4000; pid = 0:0
20:54:28.689 0 D vector_length = 3500; pid = 0:0
20:54:28.791 0 D vector_length = 3000; pid = 0:0
20:54:28.893 0 D vector_length = 2500; pid = 0:0
20:54:29.011 0 D leaving the user mode; pid = 0:0
20:54:29.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_8840, ss:rsp: 0x001B:0v7F7F_FFFF_B110, rflags: IF PF }
20:54:29.025 0 I dequeue; pid = Some(0:0)
20:54:29.029 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:29.043 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFBD48, rdi: 0x7F7FFFFFBD48, rsi: 0x7F7FFFFFBD57, { mode: user, cs:rip: 0x0023:0v1004_8840, ss:rsp: 0x001B:0v7F7F_FFFF_B110, rflags: IF PF } }
20:54:29.111 0 D leaving the user mode; pid = 0:0
20:54:29.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B9E8, rflags: IF ZF PF }
20:54:29.127 0 I dequeue; pid = Some(0:0)
20:54:29.131 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:29.145 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFBAD0, rsi: 0x7F7FFFFFBA88, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B9E8, rflags: IF ZF PF } }
20:54:29.211 0 D leaving the user mode; pid = 0:0
20:54:29.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B978, rflags: IF ZF PF }
20:54:29.227 0 I dequeue; pid = Some(0:0)
20:54:29.231 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:29.245 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x100B41D0, rsi: 0x7F7FFFFFBB50, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B978, rflags: IF ZF PF } }
20:54:29.009 0 D vector_length = 2000; pid = 0:0
20:54:29.111 0 D vector_length = 1500; pid = 0:0
20:54:29.311 0 D leaving the user mode; pid = 0:0
20:54:29.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_ACF0, ss:rsp: 0x001B:0v7F7F_FFFF_BCF0, rflags: IF PF }
20:54:29.325 0 I dequeue; pid = Some(0:0)
20:54:29.329 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:29.343 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x10174088, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1004_ACF0, ss:rsp: 0x001B:0v7F7F_FFFF_BCF0, rflags: IF PF } }
20:54:29.267 0 D vector_length = 1000; pid = 0:0
20:54:29.373 0 D vector_length = 500; pid = 0:0
20:54:29.411 0 D leaving the user mode; pid = 0:0
20:54:29.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_D7FB, ss:rsp: 0x001B:0v7F7F_FFFF_B308, rflags: IF }
20:54:29.425 0 I dequeue; pid = Some(0:0)
20:54:29.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:29.441 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x7F7FFFFFB2F0, rsi: 0x7F7FFFFFBE38, { mode: user, cs:rip: 0x0023:0v1004_D7FB, ss:rsp: 0x001B:0v7F7F_FFFF_B308, rflags: IF } }
20:54:29.511 0 D leaving the user mode; pid = 0:0
20:54:29.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B7D8, rflags: IF ZF PF }
20:54:29.527 0 I dequeue; pid = Some(0:0)
20:54:29.531 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:29.545 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFB900, rsi: 0x7F7FFFFFB870, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_B7D8, rflags: IF ZF PF } }
20:54:29.611 0 D leaving the user mode; pid = 0:0
20:54:29.615 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_B0C8, ss:rsp: 0x001B:0v7F7F_FFFF_AEA8, rflags: IF }
20:54:29.625 0 I dequeue; pid = Some(0:0)
20:54:29.629 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:29.643 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFAFA8, rdi: 0x7F7FFFFFAFA8, rsi: 0x7F7FFFFFE707, { mode: user, cs:rip: 0x0023:0v1004_B0C8, ss:rsp: 0x001B:0v7F7F_FFFF_AEA8, rflags: IF } }
20:54:29.711 0 D leaving the user mode; pid = 0:0
20:54:29.715 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_0850, ss:rsp: 0x001B:0v7F7F_FFFF_B070, rflags: IF }
20:54:29.725 0 I dequeue; pid = Some(0:0)
20:54:29.729 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:29.743 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFFB080, rdi: 0x7F7FFFFFB080, rsi: 0x1008ECE0, { mode: user, cs:rip: 0x0023:0v1004_0850, ss:rsp: 0x001B:0v7F7F_FFFF_B070, rflags: IF } }
20:54:29.811 0 D leaving the user mode; pid = 0:0
20:54:29.815 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1007_C89C, ss:rsp: 0x001B:0v7F7F_FFFF_7408, rflags: IF ZF PF }
20:54:29.827 0 I dequeue; pid = Some(0:0)
20:54:29.831 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:29.845 0 D entering the user mode; pid = 0:0; registers = { rax: 0x38A800000000FFC6, rdi: 0x7F7FFFFF7688, rsi: 0x3A, { mode: user, cs:rip: 0x0023:0v1007_C89C, ss:rsp: 0x001B:0v7F7F_FFFF_7408, rflags: IF ZF PF } }
20:54:29.911 0 D leaving the user mode; pid = 0:0
20:54:29.915 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_7742, ss:rsp: 0x001B:0v7F7F_FFFF_8BF8, rflags: IF PF }
20:54:29.925 0 I dequeue; pid = Some(0:0)
20:54:29.929 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:29.943 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF8C48, rdi: 0x7F7FFFFF8C48, rsi: 0x7F7FFFFDF000, { mode: user, cs:rip: 0x0023:0v1004_7742, ss:rsp: 0x001B:0v7F7F_FFFF_8BF8, rflags: IF PF } }
20:54:30.011 0 D leaving the user mode; pid = 0:0
20:54:30.015 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_06E1, ss:rsp: 0x001B:0v7F7F_FFFF_7C98, rflags: IF PF }
20:54:30.025 0 I dequeue; pid = Some(0:0)
20:54:30.029 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:30.043 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x7F7FFFFFB910, rsi: 0x7F7FFFFFB8C8, { mode: user, cs:rip: 0x0023:0v1004_06E1, ss:rsp: 0x001B:0v7F7F_FFFF_7C98, rflags: IF PF } }
20:54:30.111 0 D leaving the user mode; pid = 0:0
20:54:30.115 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1003_AD90, ss:rsp: 0x001B:0v7F7F_FFFF_8C20, rflags: IF PF }
20:54:30.125 0 I dequeue; pid = Some(0:0)
20:54:30.129 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:30.143 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1008B368, rdi: 0x7F7FFFFF8C40, rsi: 0x7F7FFFFFB8C8, { mode: user, cs:rip: 0x0023:0v1003_AD90, ss:rsp: 0x001B:0v7F7F_FFFF_8C20, rflags: IF PF } }
20:54:29.479 0 D vector_length = 0; pid = 0:0
20:54:29.481 0 D contents_sum = 333283335000; pop_sum = 333283335000; pid = 0:0
20:54:30.171 0 I syscall = "sched_yield"; pid = 0:0
20:54:30.173 0 D leaving the user mode; pid = 0:0
20:54:30.177 0 I dequeue; pid = Some(0:0)
20:54:30.181 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:30.195 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1008B368, rdi: 0x7F7FFFFF8C40, rsi: 0x7F7FFFFFB8C8, { mode: user, cs:rip: 0x0023:0v1002_C7E8, ss:rsp: 0x001B:0v7F7F_FFFF_B998, rflags: IF PF } }
20:54:30.211 0 D leaving the user mode; pid = 0:0
20:54:30.215 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_9F48, rflags: IF ZF PF }
20:54:30.227 0 I dequeue; pid = Some(0:0)
20:54:30.231 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:30.245 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFFA098, rsi: 0x7F7FFFFFA1B8, { mode: user, cs:rip: 0x0023:0v1005_A315, ss:rsp: 0x001B:0v7F7F_FFFF_9F48, rflags: IF ZF PF } }
20:54:30.311 0 D leaving the user mode; pid = 0:0
20:54:30.315 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1008_87F0, ss:rsp: 0x001B:0v7F7F_FFFF_8830, rflags: IF PF }
20:54:30.327 0 I dequeue; pid = Some(0:0)
20:54:30.329 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:30.343 0 D entering the user mode; pid = 0:0; registers = { rax: 0x4, rdi: 0x0, rsi: 0x409, { mode: user, cs:rip: 0x0023:0v1008_87F0, ss:rsp: 0x001B:0v7F7F_FFFF_8830, rflags: IF PF } }
20:54:30.411 0 D leaving the user mode; pid = 0:0
20:54:30.415 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1006_7180, ss:rsp: 0x001B:0v7F7F_FFFF_8810, rflags: IF PF }
20:54:30.425 0 I dequeue; pid = Some(0:0)
20:54:30.429 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:30.443 0 D entering the user mode; pid = 0:0; registers = { rax: 0x1, rdi: 0x7F7FFFFF8A67, rsi: 0x1, { mode: user, cs:rip: 0x0023:0v1006_7180, ss:rsp: 0x001B:0v7F7F_FFFF_8810, rflags: IF PF } }
20:54:30.511 0 D leaving the user mode; pid = 0:0
20:54:30.515 0 I the process was preempted; pid = 0:0; user_context = { mode: user, cs:rip: 0x0023:0v1004_0D80, ss:rsp: 0x001B:0v7F7F_FFFF_78D0, rflags: IF }
20:54:30.525 0 I dequeue; pid = Some(0:0)
20:54:30.529 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:30.543 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF7E78, rdi: 0x7F7FFFFF7918, rsi: 0x7F7FFFFF7917, { mode: user, cs:rip: 0x0023:0v1004_0D80, ss:rsp: 0x001B:0v7F7F_FFFF_78D0, rflags: IF } }
20:54:30.601 0 I syscall = "sched_yield"; pid = 0:0
20:54:30.605 0 D leaving the user mode; pid = 0:0
20:54:30.609 0 I dequeue; pid = Some(0:0)
20:54:30.611 0 I switch to; address_space = "0:0" @ 0p7E2_6000
20:54:30.625 0 D entering the user mode; pid = 0:0; registers = { rax: 0x7F7FFFFF7E78, rdi: 0x7F7FFFFF7918, rsi: 0x7F7FFFFF7917, { mode: user, cs:rip: 0x0023:0v1002_C7E8, ss:rsp: 0x001B:0v7F7F_FFFF_B998, rflags: IF } }
20:54:30.659 0 E failed to log an event; error = RingBuffer(Overflow { capacity: 16380, len: 16380, exceeding_object_len: 1 }); timestamp = 2023-11-14 20:54:29.763525451 UTC; metadata = Metadata { name: "event user/lib/src/allocator/mod.rs:34", target: "lib::allocator", level: Level(Debug), module_path: "lib::allocator", location: user/lib/src/allocator/mod.rs:34, fields: {allocator_info}, callsite: Identifier(0x100b4080), kind: Kind(EVENT) }; message_prefix = "allocator_info = { valid: true, total: { allocations: 40297 - 40297 = 0, requested: 82.336 MiB - 82.336 MiB = 0 B, allocated: 99"; pid = 0:0
20:54:30.665 0 D end_info = { allocations: 40297 - 40297 = 0, requested: 82.336 MiB - 82.336 MiB = 0 B, allocated: 99.992 MiB - 99.992 MiB = 0 B, pages: 15254 - 14222 = 1032, loss: 4.031 MiB = 100.000% }; pid = 0:0
20:54:30.669 0 E lost some log messages; recent = 1; total = 4; plan_b_failures = 0; plan_c_failures = 0; pid = 0:0
20:54:30.669 0 D end_info_diff = { allocations: 10031 - 10031 = 0, requested: 462.930 KiB - 462.930 KiB = 0 B, allocated: 462.930 KiB - 462.930 KiB = 0 B, pages: 113 - 94 = 19, loss: 76.000 KiB = 100.000% }; pid = 0:0
20:54:30.735 0 I free; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_6000, { rip: 0v1002_C7E8, rsp: 0v7F7F_FFFF_B998 } }; process_count = 0
20:54:30.741 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 17479, unlocks: 17479, waits: 0 }
20:54:30.749 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 153, unlocks: 153, waits: 0 }
20:54:30.755 0 I switch to; address_space = "base" @ 0p1000
20:54:30.759 0 I drop the current address space; address_space = "0:0" @ 0p7E2_6000; switch_to = "base" @ 0p1000
20:54:31.843 0 I syscall = "exit"; pid = 0:0; code = 0; reason = Ok(Ok)
20:54:31.849 0 D leaving the user mode; pid = 0:0
20:54:31.851 0 I dequeue; pid = None
5_um_2_memory_allocator::user_space_memory_allocator [passed]
20:54:31.859 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```

Как видно, тест `user_space_memory_allocator()` запускает те же проверки,
что и тесты `basic()`, `alignment()`, `grow_and_shrink()` и `stress()` из
[задачи про аллокатор памяти общего назначения в ядре](../../lab/book/3-smp-1-memory-allocator-1-big.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-1--%D0%B0%D0%BB%D0%BB%D0%BE%D0%BA%D0%B0%D1%82%D0%BE%D1%80-%D0%B1%D0%BE%D0%BB%D1%8C%D1%88%D0%B8%D1%85-%D0%B1%D0%BB%D0%BE%D0%BA%D0%BE%D0%B2-%D0%BF%D0%B0%D0%BC%D1%8F%D1%82%D0%B8).
Но делает это в пространстве пользователя.


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/process/syscall.rs | 185 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++--------
 1 file changed, 167 insertions(+), 18 deletions(-)
```
