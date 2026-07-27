## Index node (inode)

В [inode](https://en.wikipedia.org/wiki/Inode) хранится метаинформация об объекте с данными.
Имя объекта не хранится, оно хранится в директории.
Объектом может быть как файл, так и директория.
В случае директорий, их содержимое --- массив из имен дочерних объектов и их номеров
[inode](https://en.wikipedia.org/wiki/Inode).
Номер [inode](https://en.wikipedia.org/wiki/Inode) корневой директории файловой системы ---
константа
[`kernel::fs::superblock::Superblock::ROOT_INODE`](../../doc/kernel/fs/superblock/struct.Superblock.html#associatedconstant.ROOT_INODE),
доступная через метод
[`kernel::fs::superblock::Superblock::root()`](../../doc/kernel/fs/superblock/struct.Superblock.html#method.root).

Структура `Inode` выглядит так:

```rust
{{#include ../../kernel/src/fs/inode.rs:inode}}

{{#include ../../kernel/src/fs/inode.rs:kind}}

{{#include ../../kernel/src/fs/inode.rs:forest}}

{{#include ../../kernel/src/fs/inode.rs:max_height}}
```


### Блоки данных inode

Блоки данных
[inode](https://en.wikipedia.org/wiki/Inode)
адресуются через массив `root_blocks` структуры `struct Inode`.

![Блоки данных файла](6-fs-3-inode.svg)


### Задача 3 --- index node (inode)

Реализуйте недостающие методы структуры
[`kernel::fs::inode::Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
в файле
[`kernel/src/fs/inode.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/fs/inode.rs).
При записи в
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
обновляйте время его модификации ---
[`Inode::modify_time`](../../doc/kernel/fs/inode/struct.Inode.html#structfield.modify_time).

Вам могут пригодиться методы
[`usize::div_ceil()`](https://doc.rust-lang.org/nightly/core/primitive.usize.html#method.div_ceil) и
[`usize::next_multiple_of()`](https://doc.rust-lang.org/nightly/core/primitive.usize.html#method.next_multiple_of).


#### [`kernel::fs::inode::Inode::block_entry()`](../../doc/kernel/fs/inode/struct.Inode.html#method.block_entry)

Вспомогательный [метод](../../doc/kernel/fs/inode/struct.Inode.html#method.block_entry)

```rust
{{#include ../../kernel/src/fs/inode.rs:block_entry}}
```

по номеру блока `inode_block_number` внутри данных
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
возвращает ссылку на запись из метаданных этого
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html).
Эта запись предназначена для номера блока на диске, хранящего указанный блок данных
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html).
С помощью этого метода происходит отображение смещения внутри данных файла в номер блока на диске,
хранящего эти данные.
Номер `inode_block_number` равен смещению внутри данных файла, делённому на размер блока.
Нужная запись находится либо непосредственно в
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html),
и тогда это `&mut Inode::root_blocks[0]`.
Либо в одном из листьевых косвенных блоков леса
[`Inode::root_blocks`](../../doc/kernel/fs/inode/struct.Inode.html#structfield.root_blocks).

- Если при обходе леса встречается не выделенный косвенный блок, то его нужно выделить из `bitmap`. Не выделенные блоки имеют зарезервированный номер [`kernel::fs::inode::NO_BLOCK`](../../doc/kernel/fs/inode/constant.NO_BLOCK.html). Если при этом `bitmap` равен `None`, верните ошибку [`Error::NoDisk`](../../doc/kernel/error/enum.Error.html#variant.NoDisk).
- А вот выделять блок для данных, на который указывает результирующая запись, не нужно.

Этот метод похож на
[`kernel::memory::mapping::Mapping::translate()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.translate),
[который вы уже реализовали](../../lab/book/2-mm-6-address-space-2-translate.html#%D0%9E%D1%82%D0%BE%D0%B1%D1%80%D0%B0%D0%B6%D0%B5%D0%BD%D0%B8%D0%B5-%D0%B2%D0%B8%D1%80%D1%82%D1%83%D0%B0%D0%BB%D1%8C%D0%BD%D1%8B%D1%85-%D1%81%D1%82%D1%80%D0%B0%D0%BD%D0%B8%D1%86-%D0%BD%D0%B0-%D1%84%D0%B8%D0%B7%D0%B8%D1%87%D0%B5%D1%81%D0%BA%D0%B8%D0%B5-%D1%84%D1%80%D0%B5%D0%B9%D0%BC%D1%8B).
Только в
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
для блоков не одно дерево фиксированной высоты, а последовательность деревьев возрастающей высоты.

Вы можете использовать вспомогательные методы работы с лесом:

```rust
{{#include ../../kernel/src/fs/inode.rs:find_leaf}}

{{#include ../../kernel/src/fs/inode.rs:remove_tree}}

{{#include ../../kernel/src/fs/inode.rs:traverse}}

{{#include ../../kernel/src/fs/inode.rs:next_level}}
```

Или же структурировать код по-своему.

Дополнительный [метод](../../doc/kernel/fs/inode/struct.Inode.html#method.block)

```rust
{{#include ../../kernel/src/fs/inode.rs:block}}
```

оборачивает реализованный вами
[`Inode::block_entry()`](../../doc/kernel/fs/inode/struct.Inode.html#method.block_entry)
и возвращает блок в памяти блочного кеша, где хранится заданный блок
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html).


#### [`kernel::fs::inode::Inode::set_size()`](../../doc/kernel/fs/inode/struct.Inode.html#method.set_size)

[Метод](../../doc/kernel/fs/inode/struct.Inode.html#method.set_size)

```rust
{{#include ../../kernel/src/fs/inode.rs:set_size}}
```

изменяет размер данных
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
в большую или меньшую сторону.
При этом он должен отдавать в `bitmap` не используемые больше блоки данных.
Если новый размер `size` равен `0`, то все косвенные блоки также нужно освободить.
Так как с таким аргументом этот метод вызывается при удалении
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html).
А при удалении, естественно, нужно освободить все ресурсы.
Также имеет смысл освобождать хотя бы часть не нужных более косвенных блоков.
Например, можно освобождать не нужные более деревья леса блоков целиком.

Если файл расширяется, то новые блоки с данными должны содержать нули.
Как вариант, вы можете не хранить такие блоки на диске, заменяя их номера на
[`NO_BLOCK`](../../doc/kernel/fs/inode/constant.NO_BLOCK.html).
И выделять реальные блоки только в момент записи.
Косвенные блоки тоже можно выделять лениво в момент записи.
Ленивый вариант породит
[разрежённый файл](https://ru.wikipedia.org/wiki/%D0%A0%D0%B0%D0%B7%D1%80%D0%B5%D0%B6%D1%91%D0%BD%D0%BD%D1%8B%D0%B9_%D1%84%D0%B0%D0%B9%D0%BB).

Вам могут пригодиться:

- [`Inode::block_entry()`](../../doc/kernel/fs/inode/struct.Inode.html#method.block_entry).
- Функция [`core::cmp::min()`](https://doc.rust-lang.org/nightly/core/cmp/fn.min.html).
- Метод [`slice::fill()`](https://doc.rust-lang.org/nightly/core/primitive.slice.html#method.fill) [срезов](https://doc.rust-lang.ru/book/ch04-03-slices.html). Заполнять нулями имеет смысл не побайтно, а целыми [`u64`](https://doc.rust-lang.org/nightly/core/primitive.u64.html) или [`usize`](https://doc.rust-lang.org/nightly/core/primitive.usize.html).


#### [`kernel::fs::inode::Inode::read()`](../../doc/kernel/fs/inode/struct.Inode.html#method.read)

[Метод](../../doc/kernel/fs/inode/struct.Inode.html#method.read)

```rust
{{#include ../../kernel/src/fs/inode.rs:read}}
```

читает из данных
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
по смещению `offset` в буфер `buffer` столько байт,
сколько остаётся до конца файла или до конца буфера.
И возвращает количество прочитанных байт.
Если
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
не является файлом, верните ошибку
[`Error::NotFile`](../../doc/kernel/error/enum.Error.html#variant.NotFile).
Если `offset` превышает размер файла, верните ошибку
[`Error::InvalidArgument`](../../doc/kernel/error/enum.Error.html#variant.InvalidArgument).
Если `offset` равен размеру файла, верните `0` прочитанных байт.

Вам могут пригодиться:

- [`Inode::block()`](../../doc/kernel/fs/inode/struct.Inode.html#method.block).
- Функция [`core::cmp::min()`](https://doc.rust-lang.org/nightly/core/cmp/fn.min.html).
- Метод [`slice::copy_from_slice()`](https://doc.rust-lang.org/nightly/core/primitive.slice.html#method.copy_from_slice) [срезов](https://doc.rust-lang.ru/book/ch04-03-slices.html).


#### [`kernel::fs::inode::Inode::write()`](../../doc/kernel/fs/inode/struct.Inode.html#method.write)

[Метод](../../doc/kernel/fs/inode/struct.Inode.html#method.read)

```rust
{{#include ../../kernel/src/fs/inode.rs:write}}
```

записывает в данные
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
по смещению `offset` байты из буфера `buffer`.
При необходимости расширяет размер файла.
И возвращает количество записанных байт.
Если
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
не является файлом, верните ошибку [`Error::NotFile`](../../doc/kernel/error/enum.Error.html#variant.NotFile).
Если `offset` превышает размер файла, --- это нормальная ситуация.
Расширьте файл нулями до заданного `offset`.

Вам могут пригодиться:

- [`Inode::block()`](../../doc/kernel/fs/inode/struct.Inode.html#method.block).
- [`Inode::set_size()`](../../doc/kernel/fs/inode/struct.Inode.html#method.set_size).
- Функция [`core::cmp::min()`](https://doc.rust-lang.org/nightly/core/cmp/fn.min.html).
- Метод [`slice::copy_from_slice()`](https://doc.rust-lang.org/nightly/core/primitive.slice.html#method.copy_from_slice) [срезов](https://doc.rust-lang.ru/book/ch04-03-slices.html).


### Проверьте себя

Теперь должны заработать тесты в файле
[`kernel/tests/6-fs-3-inode.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/6-fs-3-inode.rs):

```console
$ (cd kernel; cargo test --test 6-fs-3-inode)
...
6_fs_3_inode::big_file--------------------------------------
17:12:26 0 I formatted the file system; free_space = 31.859 MiB; disk = { id: 1, io_port: 0x1F0, io_disk: 1 }; block_size = 4.000 KiB; block_count = 8192; blocks = 36..8192; inode_size = 64 B; inodes = 3..2048; directory_entry_size = 128 B; max_file_size = 513.002 GiB
17:12:52.473 0 D file_block_count = 8074; file_size = 31.539 MiB
17:14:01.305 0 D free_block_count = 65; used_block_count = 8091
17:14:01.513 0 D block_cache_stats = Stats { discards: 8, evictions: 23319, reads: 24343, writes: 31402 }
6_fs_3_inode::big_file----------------------------- [passed]

6_fs_3_inode::block_entry-----------------------------------
17:14:03.125 0 D block_count = 8192
17:14:04.121 0 D i = 0; prev_entry = 0v0; current_entry = 0v100_001C_11F8
17:14:07.047 0 D i = 100000; prev_entry = 0v7FFF_F32E_A4F0; current_entry = 0v7FFF_F32E_A4F8
17:14:09.979 0 D i = 200000; prev_entry = 0v7FFF_F33A_D9F0; current_entry = 0v7FFF_F33A_D9F8
17:14:13.247 0 D i = 300000; prev_entry = 0v7FFF_F347_2EF0; current_entry = 0v7FFF_F347_2EF8
17:14:17.053 0 D i = 400000; prev_entry = 0v7FFF_F353_63F0; current_entry = 0v7FFF_F353_63F8
17:14:20.893 0 D i = 500000; prev_entry = 0v7FFF_F35F_98F0; current_entry = 0v7FFF_F35F_98F8
17:14:24.703 0 D i = 600000; prev_entry = 0v7FFF_F36B_DDF0; current_entry = 0v7FFF_F36B_DDF8
17:14:28.525 0 D i = 700000; prev_entry = 0v7FFF_F378_12F0; current_entry = 0v7FFF_F378_12F8
17:14:31.823 0 D block entry allocation is done, checking the entries
17:14:31.827 0 D i = 0; actual = 0; expected = 0
17:14:33.895 0 D i = 100000; actual = 100000; expected = 100000
17:14:35.965 0 D i = 200000; actual = 200000; expected = 200000
17:14:38.379 0 D i = 300000; actual = 300000; expected = 300000
17:14:41.355 0 D i = 400000; actual = 400000; expected = 400000
17:14:44.309 0 D i = 500000; actual = 500000; expected = 500000
17:14:47.201 0 D i = 600000; actual = 600000; expected = 600000
17:14:50.093 0 D i = 700000; actual = 700000; expected = 700000
17:14:52.991 0 D block_cache_stats = Stats { discards: 0, evictions: 0, reads: 1543, writes: 1577 }
6_fs_3_inode::block_entry-------------------------- [passed]

6_fs_3_inode::read_write_speed------------------------------
17:14:55.537 0 I formatted the file system; free_space = 31.859 MiB; disk = { id: 1, io_port: 0x1F0, io_disk: 1 }; block_size = 4.000 KiB; block_count = 8192; blocks = 36..8192; inode_size = 64 B; inodes = 3..2048; directory_entry_size = 128 B; max_file_size = 513.002 GiB
17:15:02.907 0 D disk read speed; throughput_per_second = 1.859 MiB; size = 10.000 MiB; elapsed = 5.381 s
17:15:04.679 0 D file system read speed; throughput_per_second = 30.076 MiB; size = 10.000 MiB; elapsed = 332.491 ms; timeout = 2.000 s
17:15:06.443 0 D file system write speed; throughput_per_second = 29.859 MiB; size = 10.000 MiB; elapsed = 334.908 ms; timeout = 2.000 s
17:15:08.651 0 D disk write speed; throughput_per_second = 4.547 MiB; size = 10.000 MiB; elapsed = 2.199 s
17:15:08.657 0 D block_cache_stats = Stats { discards: 5624, evictions: 0, reads: 2569, writes: 5134 }
6_fs_3_inode::read_write_speed--------------------- [passed]

6_fs_3_inode::set_size--------------------------------------
17:15:11.011 0 I formatted the file system; free_space = 31.859 MiB; disk = { id: 1, io_port: 0x1F0, io_disk: 1 }; block_size = 4.000 KiB; block_count = 8192; blocks = 36..8192; inode_size = 64 B; inodes = 3..2048; directory_entry_size = 128 B; max_file_size = 513.002 GiB
17:15:12.215 0 D offset = 0; size = 0
17:15:12.611 0 D offset = 1; size = 1
17:15:12.959 0 D offset = 2; size = 2
17:15:13.319 0 D offset = 1363; size = 1363
17:15:13.681 0 D offset = 1364; size = 1364
17:15:14.023 0 D offset = 1365; size = 1365
17:15:14.399 0 D offset = 1366; size = 1366
17:15:14.775 0 D offset = 1367; size = 1367
17:15:15.167 0 D offset = 2728; size = 2728
17:15:15.735 0 D offset = 2729; size = 2729
17:15:16.391 0 D offset = 2730; size = 2730
17:15:17.033 0 D offset = 2731; size = 2731
17:15:17.755 0 D offset = 2732; size = 2732
17:15:18.469 0 D offset = 4093; size = 4093
17:15:18.969 0 D offset = 4094; size = 4094
17:15:19.811 0 D offset = 4095; size = 4095
17:15:20.739 0 D offset = 4096; size = 4096
17:15:21.315 0 D offset = 4097; size = 4097
17:15:22.009 0 D offset = 5458; size = 5458
17:15:22.403 0 D offset = 5459; size = 5459
17:15:23.081 0 D offset = 5460; size = 5460
17:15:23.513 0 D offset = 5461; size = 5461
17:15:23.849 0 D offset = 5462; size = 5462
17:15:24.197 0 D offset = 6823; size = 6823
17:15:24.583 0 D offset = 6824; size = 6824
17:15:24.921 0 D offset = 6825; size = 6825
17:15:25.265 0 D offset = 6826; size = 6826
17:15:25.609 0 D offset = 6827; size = 6827
17:15:25.987 0 D offset = 8188; size = 8188
17:15:26.333 0 D offset = 8189; size = 8189
17:15:26.671 0 D offset = 8190; size = 8190
17:15:27.013 0 D offset = 8191; size = 8191
17:15:27.383 0 D offset = 8192; size = 8192
17:15:27.407 0 D block_cache_stats = Stats { discards: 0, evictions: 0, reads: 9, writes: 3111 }
6_fs_3_inode::set_size----------------------------- [passed]

6_fs_3_inode::write_read------------------------------------
17:15:29.397 0 I formatted the file system; free_space = 31.859 MiB; disk = { id: 1, io_port: 0x1F0, io_disk: 1 }; block_size = 4.000 KiB; block_count = 8192; blocks = 36..8192; inode_size = 64 B; inodes = 3..2048; directory_entry_size = 128 B; max_file_size = 513.002 GiB
17:15:32.101 0 D actual = [0, 2, 6, 12, 20, 30, 42, 56, 72, 90]; expected = [0, 2, 6, 12, 20, 30, 42, 56, 72, 90]
17:15:32.773 0 D block_cache_stats = Stats { discards: 0, evictions: 0, reads: 260, writes: 257 }
6_fs_3_inode::write_read--------------------------- [passed]
17:15:33.951 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/fs/inode.rs | 171 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++---------
 1 file changed, 155 insertions(+), 16 deletions(-)
```
