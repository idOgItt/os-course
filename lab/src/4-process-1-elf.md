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
fn ku::process::elf::load<T: BigAllocator>(
    allocator: &mut T,
    file: &[u8],
) -> Result<Virt>
```

Она принимает на вход

- Аллокатор `allocator`, реализующий типаж [`ku::allocator::big::BigAllocator`](../../doc/ku/allocator/big/trait.BigAllocator.html). Для упрощения реализации функции[`load()`](../../doc/ku/process/elf/fn.load.html), вызывающая её функция гарантирует, что загрузка происходит в текущее адресное пространство, и аллокатор работает в нём же. Таже для упрощения, флаги сегментов --- [`ProgramHeader::flags()`](../../doc/xmas_elf/program/enum.ProgramHeader.html#method.flags) --- предлагается игнорировать. Вызывающая функция должна настроить аллокатор так, чтобы он отображал выделяемую память с разрешениями на запись, исполнение и с доступом для пользователя.
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

Для загрузки сегмента файла она пользуется вспомогательной [функцией](../../doc/ku/process/elf/fn.load_program_header.html)

```rust
fn ku::process::elf::load_program_header<T: BigAllocator>(
    allocator: &mut T,
    program_header: &ProgramHeader,
    file: &[u8],
    mapped_end: &mut Virt,
) -> Result<()>
```

Функция [`load_program_header()`](../../doc/ku/process/elf/fn.load_program_header.html)
загружает в память заданный `program_header`, поддерживая `mapped_end` ---
адрес до которого (не включительно) память для загружаемого процесса уже аллоцирована.

Отслеживать конец аллоцированной части памяти `mapped_end` нужно,
так как аллоцировать и отображать память можно только постранично,
а сегменты ELF--файла могут быть невыровнены по границе страницы.
И несколько сегментов могут задевать одну и ту же страницу.
При загрузке первого из них
[`load_program_header()`](../../doc/ku/process/elf/fn.load_program_header.html)
отобразит эту страницу в память и обновит `mapped_end`.
А при загрузке последующих, по значению `mapped_end` она поймёт, что отображать страницу в память уже не нужно
и достаточно только записать в неё очередную порцию данных из ELF--файла.

Таким образом,
[`load_program_header()`](../../doc/ku/process/elf/fn.load_program_header.html)
делает две вещи:

- Расширяет отображённое в память пространство процесса.
- Копирует содержимое очередного сегмента `program_header` в память по адресу [`ProgramHeader::virtual_addr()`](../../doc/xmas_elf/program/enum.ProgramHeader.html#method.virtual_addr). Этот сегмент записан в срезе `file` по смещению [`ProgramHeader::offset()`](../../doc/xmas_elf/program/enum.ProgramHeader.html#method.offset).

Обратите внимание на то, что размер сегмента в файле
[`ProgramHeader::file_size()`](../../doc/xmas_elf/program/enum.ProgramHeader.html#method.file_size)
может быть меньше чем его размер в памяти
[`ProgramHeader::mem_size()`](../../doc/xmas_elf/program/enum.ProgramHeader.html#method.mem_size).
Тогда дополнительные байты памяти нужно занулить.
Этого требует формат ELF --- там может, например, располагаться секция
[`.bss`](https://en.wikipedia.org/wiki/.bss),
предназначенная для неинициализированных или инициализированных нулями статических переменных.

Для расширения отображения,
[`load_program_header()`](../../doc/ku/process/elf/fn.load_program_header.html)
прибегает к помощи вспомогательной
[функции](../../doc/ku/process/elf/fn.extend_mapping.html)

```rust
fn ku::process::elf::extend_mapping<T: BigAllocator>(
    allocator: &mut T,
    memory_block: &Block<Virt>,
    mapped_end: &mut Virt,
) -> Result<()>
```

Кроме уже знакомых нам аргументов, функция
[`extend_mapping()`](../../doc/ku/process/elf/fn.extend_mapping.html)
принимает

- Описатель блока виртуальной памяти `memory_block` для очередного сегмента ELF--файла.

Для вычисления `memory_block` используется вспомогательная [функция](../../doc/ku/process/elf/fn.memory_block.html)

```rust
fn ku::process::elf::memory_block(
    program_header: &ProgramHeader,
) -> Result<Block<Virt>>
```


### Задача 1 --- загрузка ELF--файла

Реализуйте указанные функции.
Вам могут пригодиться:

- Метод [`fn Block::<Virt>::enclosing() -> Block<Page>`](../../doc/ku/memory/block/struct.Block.html#method.enclosing), который для заданного блока виртуальных адресов возвращает минимальный содержащий его блок страниц виртуальной памяти.
- [`Block::<Virt>::try_into_mut_slice()`](../../doc/ku/memory/block/struct.Block.html#method.try_into_mut_slice).
- Метод [`fill()`](https://doc.rust-lang.org/nightly/core/primitive.slice.html#method.fill) [срезов](https://doc.rust-lang.ru/book/ch04-03-slices.html).
- [`Result::map_err()`](https://doc.rust-lang.org/nightly/core/result/enum.Result.html#method.map_err) для преобразования одного типа ошибки в другой.
- [`Virt::new_u64()`](../../doc/ku/memory/addr/struct.Addr.html#method.new_u64).
- [`ku::memory::size::into_usize()`](../../doc/ku/memory/size/fn.into_usize.html).


### Дополнительные материалы про ELF--файлы

- [Executable and Linkable Format](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format)
- [ELF](https://wiki.osdev.org/ELF)
- [ELF-64 Object File Format](https://www.uclibc.org/docs/elf-64-gen.pdf)


### Проверьте себя

Запустите тест `4-process-1-elf` из файла
[`kernel/tests/4-process-1-elf.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-1-elf.rs):

```console
$ (cd kernel; cargo test --test 4-process-1-elf)
...
4_process_1_elf::create_process-----------------------------
20:36:36 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:36:36 0 I duplicate; address_space = "process" @ 0p7E9_9000
20:36:36 0 I switch to; address_space = "process" @ 0p7E9_9000
20:36:36 0 D extend mapping; block = [0v1000_0000, 0v1000_8234), size 32.551 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:36:36 0 D elf loadable program header; file_block = [0v20_3000, 0v20_B234), size 32.551 KiB; memory_block = [0v1000_0000, 0v1000_8234), size 32.551 KiB
20:36:36 0 D extend mapping; block = [0v1000_9000, 0v1004_F303), size 280.753 KiB; page_block = [0v1000_9000, 0v1005_0000), size 284.000 KiB
20:36:36 0 D elf loadable program header; file_block = [0v20_B240, 0v25_2303), size 284.190 KiB; memory_block = [0v1000_8240, 0v1004_F303), size 284.190 KiB
20:36:36 0 D elf loadable program header; file_block = [0v25_2308, 0v25_23D0), size 200 B; memory_block = [0v1004_F308, 0v1004_F3D0), size 200 B
20:36:36 0 D extend mapping; block = [0v1005_0000, 0v1005_5F90), size 23.891 KiB; page_block = [0v1005_0000, 0v1005_6000), size 24.000 KiB
20:36:36 0 D elf loadable program header; file_block = [0v25_23D0, 0v25_8F68), size 26.898 KiB; memory_block = [0v1004_F3D0, 0v1005_5F90), size 26.938 KiB
20:36:36 0 I switch to; address_space = "base" @ 0p1000
20:36:36 0 I loaded ELF file; context = { rip: 0v1000_8F60, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.613 MiB; process = { pid: <current>, address_space: "process" @ 0p7E9_9000, { rip: 0v1000_8F60, rsp: 0v7F7F_FFFF_F000 } }
20:36:36 0 I user process page table entry; entry_point = 0v1000_8F60; frame = Frame(32321 @ 0p7E4_1000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:36:36 0 D process_frames = 148
20:36:36 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 1, unlocks: 1, waits: 0 }
20:36:36 0 I drop; address_space = "process" @ 0p7E9_9000
4_process_1_elf::create_process-------------------- [passed]

4_process_1_elf::create_process_failure---------------------
20:36:37.243 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:36:37.251 0 I duplicate; address_space = "process" @ 0p7E9_9000
20:36:37.255 0 I switch to; address_space = "process" @ 0p7E9_9000
20:36:37.259 0 I switch to; address_space = "base" @ 0p1000
20:36:37.263 0 I drop the current address space; address_space = "process" @ 0p7E9_9000; switch_to = "base" @ 0p1000
20:36:37.597 0 I expected a process creation failure; error = Elf("File is shorter than the first ELF header part")
4_process_1_elf::create_process_failure------------ [passed]
20:36:37.607 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/process/elf.rs | 75 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++--------
 1 file changed, 67 insertions(+), 8 deletions(-)
```
