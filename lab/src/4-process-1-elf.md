## Загрузка процесса в память

У нас ещё нет файловой системы.
Поэтому в тестах код пользовательских программ линкуется прямо в бинарник ядра макросом
[`core::include_bytes!()`](https://doc.rust-lang.org/core/macro.include_bytes.html):

```rust
const LOOP_ELF: &[u8] = include_bytes!("../../../user/loop/target/kernel/debug/loop");
process::create(LOOP_ELF);
```

Первой нашей задачей будет распарсить
[ELF--файл](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format)
с помощью библиотеки
[`xmas_elf`](../../doc/xmas_elf/index.html) и построить его образ в памяти.
Нам достаточно [простейшей реализации](https://wiki.osdev.org/ELF#Loading_ELF_Binaries),
которая поддерживает только статические ELF--файлы,
без [релокаций](https://wiki.osdev.org/ELF#Relocation), обработки
[секций](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#Section_header) и символов.
Она содержится в файле [`ku/src/process/elf.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/process/elf.rs).
Основной является [функция](../../doc/ku/process/elf/fn.load.html)

```rust
{{#include ../../ku/src/process/elf.rs:load}}
```

Она принимает на вход

- Аллокатор `allocator`, реализующий типаж [`ku::allocator::big::BigAllocator`](../../doc/ku/allocator/big/trait.BigAllocator.html). Для упрощения реализации функции[`load()`](../../doc/ku/process/elf/fn.load.html), вызывающая её функция гарантирует, что загрузка происходит в текущее адресное пространство, и аллокатор работает в нём же. Вызывающая функция должна настроить аллокатор так, чтобы его метод [`BigAllocator::flags()`](../../doc/ku/allocator/big/trait.BigAllocator.html#tymethod.flags) возвращал флаг доступа для пользователя, если он нужен. А также любые другие дополнительные флаги, которые не задаёт сам ELF--файл.
- Срез `file` с записанным в памяти ELF--файлом процесса.

Из `file` она создаёт объект [`xmas_elf::ElfFile`](../../doc/xmas_elf/struct.ElfFile.html),
проходится по его
[сегментам](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format#Program_header) итератором
[`ElfFile::program_iter()`](../../doc/xmas_elf/struct.ElfFile.html#method.program_iter)
и загружает в память те из них, у которых тип
[`ProgramHeader::get_type()`](../../doc/xmas_elf/program/enum.ProgramHeader.html#method.get_type)
является загружаемым ---
[`Type::Load`](../../doc/xmas_elf/program/enum.Type.html#variant.Load).
После загрузки она возвращает точку входа в загруженную программу
[`HeaderPt2::entry_point()`](../../doc/xmas_elf/header/enum.HeaderPt2.html#method.entry_point)
в виде виртуального адреса
[`Virt`](../../doc/ku/memory/addr/type.Virt.html).


### Вспомогательные структуры и функции

Прочитайте документацию ко всем
вспомогательным структурам и функциям в файле
[`ku/src/process/elf.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/process/elf.rs).
Вам придётся их реализовывать и использовать.

Основной является функция [`ku::process::elf::load()`](../../doc/ku/process/elf/fn.load.html),
которая загружает сегменты ELF--файла по одному,
фактически проходя по ним
[методом сканирующей прямой](https://ru.algorithmica.org/cs/decomposition/scanline/).
Событиями, которые нужно при этом обрабатывать, являются:
- Границы сегментов ELF--файла.
- Выровненные на границы страниц границы сегментов.

System V Application Binary Interface - DRAFT - 24 April 2001 [утверждает что](https://refspecs.linuxbase.org/elf/gabi4+/ch5.pheader.html):

> `PT_LOAD`
> The array element specifies a loadable segment, described by `p_filesz` and `p_memsz`. The bytes from the file are mapped to the beginning of the memory segment. If the segment's memory size (`p_memsz`) is larger than the file size (`p_filesz`), the "extra" bytes are defined to hold the value `0` and to follow the segment's initialized area. The file size may not be larger than the memory size. Loadable segment entries in the program header table appear in ascending order, sorted on the `p_vaddr` member.

Поэтому можно полагаться на то что сегменты файла не пересекаются в памяти и расположены в таблице сегментов
ELF--файла в порядке адресов в памяти.
Если это не так, можно вернуть ошибку
[`Error::InvalidArgument`](../../doc/kernel/error/enum.Error.html#variant.InvalidArgument).

Трудности создаёт то, что границы сегментов могут быть не выровнены по границам страниц,
разные сегменты могут иметь разные флаги, с которыми ELF--файл предписывает их отображать ---
чтение, запись, выполнение кода.
Но страничное отображение позволяет выставлять флаги только постранично.
Поэтому для страниц, в которые попадают несколько сегментов,
придётся установить такие флаги, что они удовлетворяют требованиям всех этих сегментов.

Поэтому нам придётся отслеживать пару диапазонов адресов --- `curr` и `next`,
каждый со своими флагами.
Для их хранения используется структура
[`ku::process::elf::ElfRange`](../../doc/ku/process/elf/struct.ElfRange.html):

```rust
{{#include ../../ku/src/process/elf.rs:elf_range}}
```

Создаётся она изначально для отдельного сегмента ELF--файла
[`xmas_elf::program::ProgramHeader`](../../doc/xmas_elf/program/enum.ProgramHeader.html).
Но в процессе загрузки ELF--файла может быть как объединена со смежными диапазонами,
так и разделена на части типа
[`ku::process::elf::PageRange`](../../doc/ku/process/elf/struct.PageRange.html)
и
[`ku::process::elf::ElfRange`](../../doc/ku/process/elf/struct.ElfRange.html).

Структура [`ku::process::elf::PageRange`](../../doc/ku/process/elf/struct.PageRange.html)
описывает целый диапазон страниц,
который уже не будет пересекаться с последующими сегментами:

```rust
{{#include ../../ku/src/process/elf.rs:page_range}}
```

Поэтому требующиеся для него флаги отображения уже вычислены.
И для этого диапазона уже можно установить в страничном отображении финальные флаги доступа.

Основная функция, которая объединяет или разбивает соседние диапазоны
`curr` и `next` ---
[`ku::process::elf::combine()`](../../doc/ku/process/elf/fn.combine.html):

```rust
{{#include ../../ku/src/process/elf.rs:combine}}
```

Метод
[`ku::process::elf::ElfRange::copy_to_memory()`](../../doc/ku/process/elf/struct.ElfRange.html#method.copy_to_memory)
копирует байты сегмента из ELF--файла в память:

```rust
{{#include ../../ku/src/process/elf.rs:copy_to_memory}}
```

Он используется пока
[`ku::process::elf::ElfRange`](../../doc/ku/process/elf/struct.ElfRange.html)
ещё соответствует одному сегменту ELF--файла.
Обратите внимание на то, что размер сегмента в файле
[`ProgramHeader::file_size()`](../../doc/xmas_elf/program/enum.ProgramHeader.html#method.file_size)
может быть меньше чем его размер в памяти
[`ProgramHeader::mem_size()`](../../doc/xmas_elf/program/enum.ProgramHeader.html#method.mem_size).
Тогда дополнительные байты памяти нужно занулить.
Этого требует формат ELF --- там может, например, располагаться секция
[`.bss`](https://en.wikipedia.org/wiki/.bss),
предназначенная для неинициализированных или инициализированных нулями статических переменных.

Занулять байты имеет смысл не в методе
[`ku::process::elf::ElfRange::copy_to_memory()`](../../doc/ku/process/elf/struct.ElfRange.html#method.copy_to_memory),
а раньше, в методе
[`ku::process::elf::Loader::extend_mapping()`](../../doc/ku/process/elf/struct.Loader.html#method.extend_mapping) ---
сразу после выделения страниц.
Так как сегменты могут не покрывать выделяемые страницы полностью,
и если оставить мусор в не относящихся к сегментам местах,
в процесс пользователя может попасть закрытая от него информация.


### Структура загрузчика ELF--файла

Для загрузки файла используется вспомогательная структура
[`ku::process::elf::Loader`](../../doc/ku/process/elf/struct.Loader.html)
и её методы:

```rust
{{#include ../../ku/src/process/elf.rs:loader}}
```

Метод
[`Loader::load_program_header()`](../../doc/ku/process/elf/struct.Loader.html#method.load_program_header)
загружает в память диапазон `next` и финализирует флаги отображения части диапазона `curr`:

```rust
{{#include ../../ku/src/process/elf.rs:load_program_header}}
```

Метод
[`Loader::extend_mapping()`](../../doc/ku/process/elf/struct.Loader.html#method.extend_mapping)
расширяет отображение текущего адресного пространства, чтобы можно было записать
из ELF--файла в память блок, который описывает `next`:

```rust
{{#include ../../ku/src/process/elf.rs:extend_mapping}}
```

Метод
[`Loader::process_boundary()`](../../doc/ku/process/elf/struct.Loader.html#method.process_boundary)
обрабатывает границу между диапазонами `curr` и `next`:

```rust
{{#include ../../ku/src/process/elf.rs:process_boundary}}
```

Делает он это с помощью уже знакомой нам функции
[`ku::process::elf::combine()`](../../doc/ku/process/elf/fn.combine.html)
и с помощью метода
[`Loader::finalize_mapping()`](../../doc/ku/process/elf/struct.Loader.html#method.finalize_mapping).
Последний исправляет флаги отображения диапазона `page_range`
на запрошенные в тех сегментах ELF--файла, что пересекаются с `page_range`:

```rust
{{#include ../../ku/src/process/elf.rs:finalize_mapping}}
```


### Задача 1 --- загрузка ELF--файла

Реализуйте указанные функции.
Вам могут пригодиться:

- Вспомогательные функции [`ku::process::elf::combine_flags()`](../../doc/ku/process/elf/fn.combine_flags.html) и [`ku::process::elf::validate_order()`](../../doc/ku/process/elf/fn.validate_order.html).
- Метод [`fn Block::<Virt>::enclosing() -> Block<Page>`](../../doc/ku/memory/block/struct.Block.html#method.enclosing), который для заданного блока виртуальных адресов возвращает минимальный содержащий его блок страниц виртуальной памяти.
- [`Block::<Virt>::try_into_mut_slice()`](../../doc/ku/memory/block/struct.Block.html#method.try_into_mut_slice), а также другие методы блоков, например [`Block::is_empty()`](../../doc/ku/memory/block/struct.Block.html#method.is_empty), [`Block::is_disjoint()`](../../doc/ku/memory/block/struct.Block.html#method.is_disjoint), [`Block::count()`](../../doc/ku/memory/block/struct.Block.html#method.count), [`Block::tail()`](../../doc/ku/memory/block/struct.Block.html#method.tail).
- Метод [`MaybeUninit::fill()`](https://doc.rust-lang.org/nightly/core/mem/union.MaybeUninit.html#method.fill), инициализирующий сырую память.
- Методы [`copy_from_slice()`](https://doc.rust-lang.org/nightly/core/primitive.slice.html#method.copy_from_slice) и [`get()`](https://doc.rust-lang.org/nightly/core/primitive.slice.html#method.get) [срезов](https://doc.rust-lang.ru/book/ch04-03-slices.html).
- Арифметические операции с проверкой переполнения, такие как [`checked_add()`](https://doc.rust-lang.org/nightly/core/primitive.usize.html#method.checked_add).
- [`Result::map_err()`](https://doc.rust-lang.org/nightly/core/result/enum.Result.html#method.map_err) для преобразования одного типа ошибки в другой.
- [`Virt::new_u64()`](../../doc/ku/memory/addr/struct.Addr.html#method.new_u64).
- [`ku::memory::size::into_usize()`](../../doc/ku/memory/size/fn.into_usize.html).


### Дополнительные материалы про ELF--файлы

- [Executable and Linkable Format](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format).
- [ELF](https://wiki.osdev.org/ELF).
- [ELF-64 Object File Format](https://www.uclibc.org/docs/elf-64-gen.pdf).
- [ELF Format Cheatsheet](https://gist.github.com/x0nu11byt3/bcb35c3de461e5fb66173071a2379779).


### Проверьте себя

Запустите тест `4-process-1-elf` из файла
[`ku/tests/4-process-1-elf.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/4-process-1-elf.rs):

```console
$ (cd ku; cargo test --test 4-process-1-elf -- --test-threads=1)
...
running 7 tests
test file_offset ... 2024-10-27T20:51:22.133388Z i file_offset elf_range={ memory: [0v7953_675F_F09D, 0v7953_675F_F0A0), size 3 B, flags: 0----------(0x8000000000000000), file_range: 0x7..0xA } program_header=ProgramHeader64 { type_: Ok(Null), flags: Flags(0), offset: 7, virtual_addr: 133399123587229, physical_addr: 0, file_size: 3, mem_size: 3, align: 0 }
2024-10-27T20:51:22.133555Z :    file_offset ELF loadable program header file_block=[0v7953_675F_F085, 0v7953_675F_F088), size 3 B memory_block=[0v7953_675F_F09D, 0v7953_675F_F0A0), size 3 B
ok
...
2024-10-27T20:51:22.134522Z ! validate_order ELF loadable program headers intersect or are out of order by their virtual addresses curr_program_header=[0v1, 0v2), size 1 B next_program_header=[0v0, 0v3), size 3 B
2024-10-27T20:51:22.134571Z i validate_order curr={ memory: [0v0, 0v3), size 3 B, flags: 0------U-PX(0x5), file_range: 0x0..0x0 } next={ memory: [0v1, 0v2), size 1 B, flags: 0------U-PX(0x5), file_range: 0x0..0x0 }
2024-10-27T20:51:22.134630Z ! validate_order ELF loadable program headers intersect or are out of order by their virtual addresses curr_program_header=[0v0, 0v3), size 3 B next_program_header=[0v1, 0v2), size 1 B
ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.40s
```

А также запустите тест `4-process-1-elf` из файла
[`kernel/tests/4-process-1-elf.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-1-elf.rs):

```console
$ (cd kernel; cargo test --test 4-process-1-elf)
...
4_process_1_elf::create_process-----------------------------
20:51:47 0 I duplicate; address_space = "process" @ 0p7E9_9000
20:51:47 0 I switch to; address_space = "process" @ 0p7E9_9000
20:51:47 0 D loading ELF from file; file_block = [0v20_2000, 0v80_0970), size 5.994 MiB
20:51:47 0 D ELF program header; next = { memory: [0v1000_0000_0000, 0v1000_0000_6DE4), size 27.473 KiB, flags: 0----------(0x8000000000000000), file_range: 0x1000..0x7DE4 }
20:51:47 0 D extend mapping; block = [0v1000_0000_0000, 0v1000_0000_6DE4), size 27.473 KiB; page_block = [0v1000_0000_0000, 0v1000_0000_7000), size 28.000 KiB; flags = 0------UWPX(0x7)
20:51:47 0 D ELF loadable program header; file_block = [0v20_3000, 0v20_9DE4), size 27.473 KiB; memory_block = [0v1000_0000_0000, 0v1000_0000_6DE4), size 27.473 KiB
20:51:47 0 D ELF program header; next = { memory: [0v1000_0000_6DF0, 0v1000_0004_AC16), size 271.537 KiB, flags: 0---------X(0x0), file_range: 0x7DF0..0x4BC16 }
20:51:47 0 D extend mapping; block = [0v1000_0000_7000, 0v1000_0004_AC16), size 271.021 KiB; page_block = [0v1000_0000_7000, 0v1000_0004_B000), size 272.000 KiB; flags = 0------UWPX(0x7)
20:51:47 0 D ELF loadable program header; file_block = [0v20_9DF0, 0v24_DC16), size 271.537 KiB; memory_block = [0v1000_0000_6DF0, 0v1000_0004_AC16), size 271.537 KiB
20:51:47 0 D curr_block = [0v1000_0000_0000, 0v1000_0000_6000), size 24.000 KiB; next_block = [0v1000_0000_6000, 0v1000_0004_B000), size 276.000 KiB; is_disjoint = true
20:51:47 0 D next_block = [0v1000_0000_6000, 0v1000_0004_B000), size 276.000 KiB; next = ElfRange { file_range: 32240..310294, flags: PageTableFlags(0x0), memory: [0v1000_0000_6DF0, 0v1000_0004_AC16), size 271.537 KiB, Virt count 278054, [~16.000 TiB, ~16.000 TiB) }
20:51:47 0 D remainder = [0v1000_0000_7000, 0v1000_0004_AC16), size 271.021 KiB
20:51:47 0 D remap ELF page range; page_range = { memory: [0v1000_0000_0000, 0v1000_0000_6000), size 24.000 KiB, flags: 0----------(0x8000000000000000) }
20:51:47 0 D remap ELF page range; page_range = { memory: [0v1000_0000_6000, 0v1000_0000_7000), size 4.000 KiB, flags: 0---------X(0x0) }
20:51:47 0 D ELF program header; next = { memory: [0v1000_0004_AC18, 0v1000_0004_ACE8), size 208 B, flags: 0-------W--(0x8000000000000002), file_range: 0x4BC18..0x4BCE8 }
20:51:47 0 D ELF loadable program header; file_block = [0v24_DC18, 0v24_DCE8), size 208 B; memory_block = [0v1000_0004_AC18, 0v1000_0004_ACE8), size 208 B
20:51:47 0 D curr_block = [0v1000_0000_7000, 0v1000_0004_A000), size 268.000 KiB; next_block = [0v1000_0004_A000, 0v1000_0004_B000), size 4.000 KiB; is_disjoint = true
20:51:47 0 D remap ELF page range; page_range = { memory: [0v1000_0000_7000, 0v1000_0004_A000), size 268.000 KiB, flags: 0---------X(0x0) }
20:51:47 0 D ELF program header; next = { memory: [0v1000_0004_ACE8, 0v1000_0005_1788), size 26.656 KiB, flags: 0-------W--(0x8000000000000002), file_range: 0x4BCE8..0x52750 }
20:51:47 0 D extend mapping; block = [0v1000_0004_B000, 0v1000_0005_1788), size 25.883 KiB; page_block = [0v1000_0004_B000, 0v1000_0005_2000), size 28.000 KiB; flags = 0------UWPX(0x7)
20:51:47 0 D ELF loadable program header; file_block = [0v24_DCE8, 0v25_4750), size 26.602 KiB; memory_block = [0v1000_0004_ACE8, 0v1000_0005_1788), size 26.656 KiB
20:51:47 0 D curr_block = [0v1000_0004_A000, 0v1000_0004_A000), size 0 B; next_block = [0v1000_0004_A000, 0v1000_0005_2000), size 32.000 KiB; is_disjoint = true
20:51:47 0 D next_block = [0v1000_0004_A000, 0v1000_0005_2000), size 32.000 KiB; next = ElfRange { file_range: 310504..337744, flags: PageTableFlags(WRITABLE | NO_EXECUTE), memory: [0v1000_0004_ACE8, 0v1000_0005_1788), size 26.656 KiB, Virt count 27296, [~16.000 TiB, ~16.000 TiB) }
20:51:47 0 D remainder = [0v1000_0004_B000, 0v1000_0005_1788), size 25.883 KiB
20:51:47 0 D remap ELF page range; page_range = { memory: [0v1000_0004_B000, 0v1000_0005_2000), size 28.000 KiB, flags: 0-------W--(0x8000000000000002) }
20:51:47 0 I switch to; address_space = "base" @ 0p1000
20:51:47 0 I switch to; address_space = "process" @ 0p7E9_9000
20:51:47 0 I switch to; address_space = "base" @ 0p1000
20:51:47 0 I loaded ELF file; entry = 0v1000_0000_87F0; file_size = 5.994 MiB; process = { pid: <current>, address_space: "process" @ 0p7E9_9000, { rip: 0v1000_0000_87F0, rsp: 0v7FFF_FFFF_5000 } }
20:51:47 0 I user process page table entry; entry_point = 0v1000_0000_87F0; frame = Frame(32319 @ 0p7E3_F000); flags = PageTableFlags(PRESENT | USER_ACCESSIBLE)
20:51:47 0 D process_frames = 146
20:51:47 0 D dropping; spinlock = kernel/src/process/process.rs:83:28; stats = Stats { failures: 0, locks: 1, unlocks: 1, waits: 0 }
20:51:47 0 I drop; address_space = "process" @ 0p7E9_9000
4_process_1_elf::create_process-------------------- [passed]

4_process_1_elf::create_process_failure---------------------
20:51:49.119 0 I duplicate; address_space = "process" @ 0p7E9_9000
20:51:49.123 0 I switch to; address_space = "process" @ 0p7E9_9000
20:51:49.127 0 D loading ELF from file; file_block = [0v1, 0v1), size 0 B
20:51:49.133 0 I switch to; address_space = "base" @ 0p1000
20:51:49.137 0 I drop the current address space; address_space = "process" @ 0p7E9_9000; switch_to = "base" @ 0p1000
20:51:49.641 0 I expected a process creation failure; error = Elf("File is shorter than the first ELF header part")
4_process_1_elf::create_process_failure------------ [passed]
20:51:49.733 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/process/elf.rs | 197 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++--------
 1 file changed, 181 insertions(+), 16 deletions(-)
```
