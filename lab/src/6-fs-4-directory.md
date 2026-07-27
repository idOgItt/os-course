## Операции с директориями

Директории --- это
[inode](https://en.wikipedia.org/wiki/Inode),
с фиксированным форматом данных.
А именно, размер данных для директорий должен быть кратен размеру блока.
А сами данные --- это массив записей типа
[`kernel::fs::directory_entry::DirectoryEntry`](../../doc/kernel/fs/directory_entry/struct.DirectoryEntry.html):

```rust
struct DirectoryEntry {
    inode: usize,
    name: [u8; MAX_NAME_LEN],
}
```

Отсутствующие или удалённые записи при этом имеют зарезервированный номер `inode` равный
[`DirectoryEntry::UNUSED`](../../doc/kernel/fs/directory_entry/struct.DirectoryEntry.html#associatedconstant.UNUSED).
Проверить свободна ли запись можно методом
[`DirectoryEntry::is_free()`](../../doc/kernel/fs/directory_entry/struct.DirectoryEntry.html#method.is_free).


### Задача 4 --- операции с директориями

Реализуйте методы
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html),
которые работают с данными директорий в файле
[`kernel/src/fs/inode.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/fs/inode.rs).
Эти методы должны возвращать ошибку
[`Error::NotDirectory`](../../doc/kernel/error/enum.Error.html#variant.NotDirectory),
если вызвать их для
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
который не является директорией.


#### [`kernel::fs::inode::Iter::next()`](../../doc/kernel/fs/inode/struct.Iter.html#method.next)

Это вспомогательный [метод](../../doc/kernel/fs/inode/struct.Iter.html#method.next) для
[`kernel::fs::inode::Inode::find_entry()`](../../doc/kernel/fs/inode/struct.Inode.html#method.find_entry) и
[`kernel::fs::inode::Inode::list()`](../../doc/kernel/fs/inode/struct.Inode.html#method.list).

Он продвигает итератор

```rust
struct Iter<'a> {
    block: Block<Virt>,
    block_number: usize,
    entry: usize,
    inode: &'a mut Inode,
}
```

по записям директории
[`Iter::inode`](../../doc/kernel/fs/inode/struct.Iter.html#structfield.inode).
Поле
[`Iter::block_number`](../../doc/kernel/fs/inode/struct.Iter.html#structfield.block_number)
задаёт номер блока внутри данных
[`Inode`](../../doc/kernel/fs/inode/struct.Inode.html)
в котором находится текущая запись.
Поле
[`Iter::block`](../../doc/kernel/fs/inode/struct.Iter.html#structfield.block) ---
сам этот блок в памяти блочного кеша.
А [`Iter::entry`](../../doc/kernel/fs/inode/struct.Iter.html#structfield.entry) ---
номер текущей записи внутри блока.

Вам могут пригодиться:

- [`Inode::block()`](../../doc/kernel/fs/inode/struct.Inode.html#method.block).
- [`mem::size_of::<DirectoryEntry>()`](https://doc.rust-lang.org/nightly/core/mem/fn.size_of.html) --- размер записи директории [`DirectoryEntry`](../../doc/kernel/fs/directory_entry/struct.DirectoryEntry.html).


#### [`kernel::fs::inode::Inode::find_entry()`](../../doc/kernel/fs/inode/struct.Inode.html#method.find_entry)

Вспомогательный [метод](../../doc/kernel/fs/inode/struct.Inode.html#method.find_entry) для
[`kernel::fs::inode::Inode::find()`](../../doc/kernel/fs/inode/struct.Inode.html#method.find) и
[`kernel::fs::inode::Inode::insert()`](../../doc/kernel/fs/inode/struct.Inode.html#method.insert):

```rust
fn Inode::find_entry(
    &mut self,
    name: &str,
    bitmap: Option<&mut Bitmap>,
) -> Result<&mut DirectoryEntry>
```

Он находит запись в директории с именем `name`.
Если такой записи нет, то верните свободную запись.
Если и таких нет, и при этом `bitmap` является `Some`, попробуйте расширить директорию
и вернуть новую свободную запись.
Директорию имеет смысл расширять несколькими записями сразу на один целый блок.
Так как блок в любом случае будет выделен, его стоит сразу весь отдать под записи директории.

Чтобы не дублировать код, для обхода директории воспользуйтесь методом
[`Inode::iter()`](../../doc/kernel/fs/inode/struct.Inode.html#method.iter)
и соответствующим
[`Iter::next()`](../../doc/kernel/fs/inode/struct.Iter.html#method.next).
Также вам может пригодиться метод
[`Inode::extend()`](../../doc/kernel/fs/inode/struct.Inode.html#method.extend),
который расширяет директорию на один блок.


#### [`kernel::fs::inode::Inode::insert()`](../../doc/kernel/fs/inode/struct.Inode.html#method.insert)

Реализуйте [метод](../../doc/kernel/fs/inode/struct.Inode.html#method.insert)

```rust
fn Inode::insert(
    &mut self,
    name: &str,
    kind: Kind,
    bitmap: &mut Bitmap,
) -> Result<&mut Inode>
```

Он вставляет в директорию запись с именем `name` и типом `kind`.
Если запись с таким именем уже есть, верните ошибку
[`Error::FileExists`](../../doc/kernel/error/enum.Error.html#variant.FileExists).
Используйте любую свободную запись.
Не забудьте обновить как время модификации выделенной записи, так и время модификации самой директории.
Используйте вспомогательный метод
[`Inode::find_entry()`](../../doc/kernel/fs/inode/struct.Inode.html#method.find_entry).


### Проверьте себя

Теперь должны заработать тесты в файле
[`kernel/tests/6-fs-4-directory.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/6-fs-4-directory.rs):

```console
$ (cd kernel; cargo test --test 6-fs-4-directory)
...
6_fs_4_directory::basic_operations--------------------------
17:15:36 0 I formatted the file system; free_space = 31.859 MiB; disk = { id: 1, io_port: 0x1F0, io_disk: 1 }; block_size = 4.000 KiB; block_count = 8192; blocks = 36..8192; inode_size = 64 B; inodes = 3..2048; directory_entry_size = 128 B; max_file_size = 513.002 GiB
17:15:37 0 D list = [Entry { inode: 3, kind: File, modify_time: 2023-12-03T17:15:37Z, name: "file-1", size: 0 }]
17:15:37 0 D list = [Entry { inode: 3, kind: File, modify_time: 2023-12-03T17:15:37Z, name: "file-1", size: 1 }]
17:15:37 0 D list = [Entry { inode: 3, kind: File, modify_time: 2023-12-03T17:15:37Z, name: "file-1", size: 1 }, Entry { inode: 4, kind: File, modify_time: 2023-12-03T17:15:37Z, name: "file-2", size: 1235 }]
17:15:37 0 D block_cache_stats = Stats { discards: 0, evictions: 0, reads: 7, writes: 5 }
6_fs_4_directory::basic_operations----------------- [passed]

6_fs_4_directory::big_directory-----------------------------
17:15:39.695 0 I formatted the file system; free_space = 31.859 MiB; disk = { id: 1, io_port: 0x1F0, io_disk: 1 }; block_size = 4.000 KiB; block_count = 8192; blocks = 36..8192; inode_size = 64 B; inodes = 3..2048; directory_entry_size = 128 B; max_file_size = 513.002 GiB
17:15:41.293 0 D file_count = 100; directory_size = 16.000 KiB
17:15:41.789 0 D file_count = 200; directory_size = 28.000 KiB
17:15:42.449 0 D file_count = 300; directory_size = 40.000 KiB
17:15:43.291 0 D file_count = 400; directory_size = 52.000 KiB
17:15:44.307 0 D file_count = 500; directory_size = 64.000 KiB
17:15:45.487 0 D file_count = 600; directory_size = 76.000 KiB
17:15:46.831 0 D file_count = 700; directory_size = 88.000 KiB
17:15:48.329 0 D file_count = 800; directory_size = 100.000 KiB
17:15:49.991 0 D file_count = 900; directory_size = 116.000 KiB
17:15:51.833 0 D file_count = 1000; directory_size = 128.000 KiB
17:15:51.837 0 D free_block_count = 8123; used_block_count = 33
17:15:51.843 0 D block_cache_stats = Stats { discards: 0, evictions: 0, reads: 52, writes: 1033 }
6_fs_4_directory::big_directory-------------------- [passed]

6_fs_4_directory::max_name_len------------------------------
17:15:53.781 0 I formatted the file system; free_space = 31.859 MiB; disk = { id: 1, io_port: 0x1F0, io_disk: 1 }; block_size = 4.000 KiB; block_count = 8192; blocks = 36..8192; inode_size = 64 B; inodes = 3..2048; directory_entry_size = 128 B; max_file_size = 513.002 GiB
17:15:54.959 0 D list = [Entry { inode: 3, kind: File, modify_time: 2023-12-03T17:15:54.957Z, name: "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", size: 0 }]
17:15:54.969 0 D block_cache_stats = Stats { discards: 0, evictions: 0, reads: 4, writes: 2 }
6_fs_4_directory::max_name_len--------------------- [passed]
17:15:55.865 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/fs/inode.rs | 65 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++-------
 1 file changed, 58 insertions(+), 7 deletions(-)
```
