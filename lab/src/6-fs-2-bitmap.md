## Битмапы занятых блоков и inode


Для отслеживания какие именно элементы --- блоки или inode --- файловой системы заняты,
а какие свободны, служит структура
[`kernel::fs::bitmap::Bitmap`](../../doc/kernel/fs/bitmap/struct.Bitmap.html):

```rust
{{#include ../../kernel/src/fs/bitmap.rs:bitmap}}
```

Каждое значение в срезе
[`Bitmap::bitmap`](../../doc/kernel/fs/bitmap/struct.Bitmap.html#structfield.bitmap)
отвечает за 64 элемента.
Элементу с меньшим номером соответствует бит с меньшим весом в соответствующем элементе.
Элемент свободен тогда и только тогда, когда соответствующий ему бит равен `0`.


### Задача 2 --- битмап занятых блоков или inode

Доделайте [метод](../../doc/kernel/fs/bitmap/struct.Bitmap.html#method.format)

```rust
{{#include ../../kernel/src/fs/bitmap.rs:format}}
```

в файле
[`kernel/src/fs/bitmap.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/fs/bitmap.rs).

Вам нужно инициализировать массив `bitmap.bitmap` так,
чтобы для элементов, перечисленных в `elements`, соответствующие биты были равны нулю --- элементы свободны.
А для всех остальных --- зарезервированных и отсутствующих в битмапе элементов --- соответствующие биты были равны единице.

Реализуйте [метод](../../doc/kernel/fs/bitmap/struct.Bitmap.html#method.allocate)

```rust
{{#include ../../kernel/src/fs/bitmap.rs:allocate}}
```

в файле
[`kernel/src/fs/bitmap.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/fs/bitmap.rs).

Функция должна найти по
[`Bitmap::bitmap`](../../doc/kernel/fs/bitmap/struct.Bitmap.html#structfield.bitmap)
свободный элемент --- нулевой бит --- и аллоцировать его.

- Верните номер выделенного элемента.
- После того как новый элемент аллоцирован, сбросьте изменившийся блок самого [`Bitmap::bitmap`](../../doc/kernel/fs/bitmap/struct.Bitmap.html#structfield.bitmap) на диск с помощью реализованной вами функции [`BlockCache::flush_block()`](../../doc/kernel/fs/block_cache/struct.BlockCache.html#method.flush_block). Когда меняете метаданные, лучше сразу сбрасывать их на диск, так меньше вероятность поломки файловой системы, например, при сбое питания.
- Искать каждый раз с самого начала [`Bitmap::bitmap`](../../doc/kernel/fs/bitmap/struct.Bitmap.html#structfield.bitmap) не эффективно. Лучше, например, обходить его по циклу с того места, на котором остановились в прошлый раз. Для хранения этой позиции между вызовами [`Bitmap::allocate()`](../../doc/kernel/fs/bitmap/struct.Bitmap.html#method.allocate) служит поле [`Bitmap::cursor`](../../doc/kernel/fs/bitmap/struct.Bitmap.html#structfield.cursor).
- Проверять каждый бит тоже не эффективно. Лучше сначала найти значение в [`Bitmap::bitmap`](../../doc/kernel/fs/bitmap/struct.Bitmap.html#structfield.bitmap), в котором есть хотя бы один свободный бит. Поэтому биты сгруппированы именно по [`u64`](https://doc.rust-lang.org/nightly/core/primitive.u64.html), а не например по [`u8`](https://doc.rust-lang.org/nightly/core/primitive.u8.html).
- Верните ошибку [`Error::NoDisk`](../../doc/kernel/error/enum.Error.html#variant.NoDisk), если свободных элементов не осталось.
- [`core::ops::Range`](https://doc.rust-lang.org/nightly/core/ops/struct.Range.html) можно создать как `start..end` --- `end` при этом не включается в диапазон, --- или как `start..=end` --- в этом случае `end` включается в получившийся [`Range`](https://doc.rust-lang.org/nightly/core/ops/struct.Range.html).


### Проверьте себя

Теперь должен заработать тест `allocation()` в файле
[`kernel/tests/6-fs-2-bitmap.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/6-fs-2-bitmap.rs):

```console
$ (cd kernel; cargo test --test 6-fs-2-bitmap)
...
6_fs_2_bitmap::allocation-----------------------------------
17:12:11 0 D block_count = 8192
17:12:16.879 0 D allocated_head = [36, 37, 38, 39, 40, 41, 42, 43, 44, 45]; allocated_tail = [8182, 8183, 8184, 8185, 8186, 8187, 8188, 8189, 8190, 8191]
17:12:23.939 0 D reallocated_head = [8128, 8130, 8132, 8134, 8136, 8138, 8140, 8142, 8144, 8146]; reallocated_tail = [8108, 8110, 8112, 8114, 8116, 8118, 8120, 8122, 8124, 8126]
17:12:24.051 0 D block_cache_stats = Stats { discards: 0, evictions: 0, reads: 3, writes: 20464 }
6_fs_2_bitmap::allocation-------------------------- [passed]
17:12:24.065 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/fs/bitmap.rs | 22 ++++++++++++++++++++--
 1 file changed, 20 insertions(+), 2 deletions(-)
```
