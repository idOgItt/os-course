## Поиск файла по пути


### Задача 5 --- поиск файла по пути

Реализуйте [метод](../../doc/kernel/fs/file_system/struct.FileSystem.html#method.open)

```rust
fn FileSystem::open(
    &mut self,
    path: &str,
) -> Result<File>
```

в файле
[`kernel/src/fs/file_system.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/fs/file_system.rs).

Он должен пройти от корня файловой системы по заданному полному пути `path`.
И вернуть соответствующий
[`kernel::fs::file::File`](../../doc/kernel/fs/file/struct.File.html).
Вам пригодится метод
[`kernel::fs::inode::Inode::find()`](../../doc/kernel/fs/inode/struct.Inode.html#method.find).


### Проверьте себя

Теперь должен заработать тест `fs()` в файле
[`kernel/tests/6-fs-5-open.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/6-fs-5-open.rs):

```console
$ (cd kernel; cargo test --test 6-fs-5-open)
...
6_fs_5_open::fs---------------------------------------------
17:15:58 0 I formatted the file system; free_space = 31.859 MiB; disk = { id: 1, io_port: 0x1F0, io_disk: 1 }; block_size = 4.000 KiB; block_count = 8192; blocks = 36..8192; inode_size = 64 B; inodes = 3..2048; directory_entry_size = 128 B; max_file_size = 513.002 GiB
17:15:59 0 I path = /file-1; entry = 3, file-1, File, 5678 B = 5.545 KiB, 2023-12-03 17:15:59 UTC
17:15:59 0 I path = /file-2; entry = 5, file-2, File, 9876 B = 9.645 KiB, 2023-12-03 17:15:59 UTC
17:15:59 0 I path = /dir-1; entry = 6, dir-1, Directory, 4096 B = 4.000 KiB, 2023-12-03 17:15:59 UTC
17:15:59 0 I path = /dir-1/file-4; entry = 7, file-4, File, 0 B = 0 B, 2023-12-03 17:15:59 UTC
17:15:59 0 I path = /dir-1/file-5; entry = 8, file-5, File, 0 B = 0 B, 2023-12-03 17:15:59 UTC
17:15:59 0 I path = /dir-1/dir-2; entry = 9, dir-2, Directory, 4096 B = 4.000 KiB, 2023-12-03 17:15:59 UTC
17:15:59 0 I path = /dir-1/dir-2/dir-3; entry = 10, dir-3, Directory, 4096 B = 4.000 KiB, 2023-12-03 17:15:59 UTC
17:15:59 0 I path = /dir-1/dir-2/dir-3/file-3; entry = 11, file-3, File, 0 B = 0 B, 2023-12-03 17:15:59 UTC
17:15:59 0 I removing; path = /file-1; entry = 3, file-1, File, 5678 B = 5.545 KiB, 2023-12-03 17:15:59 UTC
17:15:59 0 I removing; path = /file-2; entry = 5, file-2, File, 9876 B = 9.645 KiB, 2023-12-03 17:15:59 UTC
17:15:59 0 I removing; path = /dir-1; entry = 6, dir-1, Directory, 4096 B = 4.000 KiB, 2023-12-03 17:15:59 UTC
17:15:59 0 I removing; path = /dir-1/file-4; entry = 7, file-4, File, 0 B = 0 B, 2023-12-03 17:15:59 UTC
17:15:59 0 I removing; path = /dir-1/file-5; entry = 8, file-5, File, 0 B = 0 B, 2023-12-03 17:15:59 UTC
17:15:59 0 I removing; path = /dir-1/dir-2; entry = 9, dir-2, Directory, 4096 B = 4.000 KiB, 2023-12-03 17:15:59 UTC
17:15:59 0 I removing; path = /dir-1/dir-2/dir-3; entry = 10, dir-3, Directory, 4096 B = 4.000 KiB, 2023-12-03 17:15:59 UTC
17:15:59 0 I removing; path = /dir-1/dir-2/dir-3/file-3; entry = 11, file-3, File, 0 B = 0 B, 2023-12-03 17:15:59 UTC
17:15:59 0 D block_cache_stats = Stats { discards: 0, evictions: 0, reads: 15, writes: 22 }
6_fs_5_open::fs------------------------------------ [passed]
17:16:00.641 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/fs/file_system.rs | 27 +++++++++++++++++++++++++--
 1 file changed, 25 insertions(+), 2 deletions(-)
```
