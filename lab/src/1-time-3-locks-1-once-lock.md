## Once lock

Прежде чем приступать к реализации
[блокировки с использованием цикла активного ожидания](https://en.wikipedia.org/wiki/Spinlock)
сделаем более простой, но тоже иногда полезный, примитив синхронизации.
Он похож на более известный

- [`pthread_once()`](https://pubs.opengroup.org/onlinepubs/7908799/xsh/pthread_once.html),
- [`std::sync::Once` из стандартной библиотеки Rust'а](https://doc.rust-lang.org/std/sync/struct.Once.html),
- [`std::call_once` из C++](https://en.cppreference.com/w/cpp/thread/call_once),
- или на [`OnceFunc` из Go](https://pkg.go.dev/sync#OnceFunc).

Но при этом чуть сложнее, так как содержит не только флаг, но и защищаемые данные.
В стандартной библиотеки Rust'а ему соответствуют два варианта, ---
[`std::sync::OnceLock`](https://doc.rust-lang.org/std/sync/struct.Once.html)
и более простой
[`std::sync::LazyLock`](https://doc.rust-lang.org/std/sync/struct.LazyLock.html).
А в стандартной библиотеке Go ---
[`OnceValue`](https://pkg.go.dev/sync#OnceValue) и
[`OnceValues`](https://pkg.go.dev/sync#OnceValues).

Решаемая им задача такова.
Пусть есть данные, которые записываются только один раз, а читаться могут много раз.
Например, глобальная константа, которую невозможно вычислить на этапе компиляции.
Тогда этот примитив при чтении возвращает либо
[`core::option::Option::None`](https://doc.rust-lang.org/nightly/core/option/enum.Option.html#variant.None),
если инициализация ещё не завершена,
либо
[`core::option::Option::Some`](https://doc.rust-lang.org/nightly/core/option/enum.Option.html#variant.Some)
с инициализированными данными.
А при записи он убеждается, что текущий писатель первый, записывает заданное значение и возвращает
[`core::result::Result::Ok`](https://doc.rust-lang.org/nightly/core/result/enum.Result.html#variant.Ok).
Внутри этот
[`Ok`](https://doc.rust-lang.org/nightly/core/result/enum.Result.html#variant.Ok)
будет содержать тип
[`()`](https://doc.rust-lang.org/nightly/core/primitive.unit.html) ---
пустой кортеж, --- тип, имеющий только одно значение.
Это аналог `void` из C и C++.
Всем писателям, кроме первого, будет возвращаться ошибка
[`core::result::Result::Err`](https://doc.rust-lang.org/nightly/core/result/enum.Result.html#variant.Err)
с кодом
[`ku::error::Error::InvalidArgument`](../../doc/kernel/error/enum.Error.html#variant.InvalidArgument)
внутри.


### Структура [`OnceLock`](../../doc/ku/sync/once_lock/struct.OnceLock.html)

В файле
[`ku/src/sync/once_lock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/once_lock.rs)
определена структура
[`OnceLock`](../../doc/ku/sync/once_lock/struct.OnceLock.html):

```rust
pub struct OnceLock<T> {
    data: UnsafeCell<MaybeUninit<T>>,
    initialized: AtomicU8,
}
```

Она параметризована произвольным типом `T`, который мы хотим защитить этой блокировкой.
Чтобы получить доступ к значению этого типа, имея переменную типа
[`OnceLock<T>`](../../doc/ku/sync/once_lock/struct.OnceLock.html),
код должен будет захватить блокировку и только после этого
сможет обратиться с содержащемуся внутри полю с защищаемыми данными типа `T`.
Без явного захвата блокировки, обратиться с защищаемому полю не получится ---
это невозможно будет выразить в коде так, чтобы он скомпилировался.
Такой подход позволяет избавиться от ошибок из-за невнимательности.

Поле
[`OnceLock::data`](../../doc/ku/sync/once_lock/struct.OnceLock.html#structfield.data)
хранит защищаемые данные.
А флаг
[`OnceLock::initialized`](../../doc/ku/sync/once_lock/struct.OnceLock.html#structfield.initialized) ---
признак их инициализированности.
До выполнения этой задачи этот флаг обёрнут в
[`spin::mutex::Mutex`](../../doc/spin/mutex/struct.Mutex.html),
а
[`OnceLock`](../../doc/ku/sync/once_lock/struct.OnceLock.html)
просто перекладывает всю свою работу на него.

То что
[`spin::mutex::Mutex`](../../doc/spin/mutex/struct.Mutex.html)
оборачивает флаг
[`OnceLock::initialized`](../../doc/ku/sync/once_lock/struct.OnceLock.html#structfield.initialized),
а не защищаемые данные
[`OnceLock::data`](../../doc/ku/sync/once_lock/struct.OnceLock.html#structfield.data) ---
**антипаттерн**.
В задаче он использован, чтобы не скрывать часть реализации,
работающую с данными.
Чтобы разбираться с тем, что такое
[`core::cell::UnsafeCell`](https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html)
и
[`core::mem::MaybeUninit`](https://doc.rust-lang.org/nightly/core/mem/union.MaybeUninit.html)
было не обязательным.
В своём же коде, всегда оборачивайте защищаемые данные в примитив синхронизации,
а не определяйте его рядом, --- слишком легко забыть захватить блокировку при обращении к данным.

Для выполнения задачи нужно будет заменить
[`spin::mutex::Mutex`](../../doc/spin/mutex/struct.Mutex.html)
на
[`core::sync::atomic::AtomicU8`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicU8.html),
или другой атомарный тип, если он подойдёт вам больше:

```rust
initialized: AtomicU8,
```

Структура
`UnsafeCell<MaybeUninit<T>>`
обеспечивает только хранение данных типа `T`,
но не взаимное исключения при доступе к этим данным.
Им в свою очередь займётся
[`OnceLock`](../../doc/ku/sync/once_lock/struct.OnceLock.html).


### Задача 2 --- реализация [`OnceLock`](../../doc/ku/sync/once_lock/struct.OnceLock.html)

Изучите [примеры использования `OnceLock`](../../doc/ku/sync/once_lock/struct.OnceLock.html#examples).

Поправьте тип поля
[`OnceLock::initialized`](../../doc/ku/sync/once_lock/struct.OnceLock.html#structfield.initialized)
на
[подходящий атомарный](https://doc.rust-lang.org/nightly/core/sync/atomic/index.html)
в определении структуры
[`OnceLock`](../../doc/ku/sync/once_lock/struct.OnceLock.html)
и в методе её создания
[`OnceLock::new()`](../../doc/ku/sync/once_lock/struct.OnceLock.html#method.new).

Реализуйте
[метод](../../doc/ku/sync/once_lock/struct.OnceLock.html#method.set)

```rust
fn OnceLock::set(
    &self,
    value: T,
) -> Result<()>
```

в файле
[`ku/src/sync/once_lock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/once_lock.rs).

Этот метод пытается захватить блокировку и инициализировать защищаемые ею данные заданным значением `value`.
Заметьте, что если попытка записи в программе ровна одна, то она должна гарантированно завершиться успешно.

В том же файле реализуйте
[метод](../../doc/ku/sync/once_lock/struct.OnceLock.html#method.is_initialized)

```rust
fn OnceLock::is_initialized(
    &self,
) -> bool
```

Он проверяет, были ли уже инициализированы защищаемые данные.
И возвращает `true`, если это так.

При решении этой задачи вам могут пригодиться:

- Метод [`core::sync::atomic::AtomicBool::compare_exchange()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.compare_exchange).
- Метод [`core::sync::atomic::AtomicBool::compare_exchange_weak()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak).
- Метод [`core::sync::atomic::AtomicBool::load()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.load).
- Метод [`core::sync::atomic::AtomicBool::store()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.store).

> В своей реализации вы можете применить
> [стандартную оптимизацию](https://en.wikipedia.org/wiki/Double-checked_locking).


### Проверьте себя

#### Обычный запуск теста [`ku/tests/1-time-2-once-lock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-2-once-lock.rs)

На этот раз тесты находятся не в `kernel`, а в `ku` ---
[`ku/tests/1-time-2-once-lock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-2-once-lock.rs).
Их можно запустить командой `cargo test --test 1-time-2-once-lock -- --test-threads=1` в директории `ku` репозитория.
Вы увидите сборку и логи запуска тестов:

```console
/.../nikka$ (cd ku; cargo test --test 1-time-2-once-lock -- --test-threads=1)
...
running 5 tests
test multiple_writers ... 2024-09-15T18:10:00.469978Z : multiple_writers iteration=0 successfully_initialized=1 initialized=372 uninitialized=1738212
2024-09-15T18:10:00.499066Z : multiple_writers iteration=1 successfully_initialized=1 initialized=249 uninitialized=3080982
2024-09-15T18:10:00.524459Z : multiple_writers iteration=2 successfully_initialized=1 initialized=328 uninitialized=875681
2024-09-15T18:10:00.552200Z : multiple_writers iteration=3 successfully_initialized=1 initialized=324 uninitialized=1518843
...
2024-09-15T18:10:03.416362Z : multiple_writers iteration=97 successfully_initialized=1 initialized=345 uninitialized=747947
2024-09-15T18:10:03.444202Z : multiple_writers iteration=98 successfully_initialized=1 initialized=326 uninitialized=1770022
2024-09-15T18:10:03.476777Z : multiple_writers iteration=99 successfully_initialized=1 initialized=308 uninitialized=3104337
ok
test set_get ... ok
test set_get_mut ... ok
test single_writer ... 2024-09-15T18:10:03.507871Z :    single_writer initialized=225 uninitialized=3639044
ok
test single_writer_is_not_obstructed ... 2024-09-15T18:10:04.668740Z : single_writer_is_not_obstructed writer_iterations=591 initialized=18160 uninitialized=44054983
2024-09-15T18:10:06.231942Z : single_writer_is_not_obstructed writer_iterations=463 initialized=16627 uninitialized=162777605
2024-09-15T18:10:07.527260Z : single_writer_is_not_obstructed writer_iterations=636 initialized=19330 uninitialized=62619146
2024-09-15T18:10:08.798899Z : single_writer_is_not_obstructed writer_iterations=572 initialized=17187 uninitialized=79984401
...
2024-09-15T18:10:30.097001Z : single_writer_is_not_obstructed writer_iterations=445 initialized=17004 uninitialized=98834564
2024-09-15T18:10:31.435031Z : single_writer_is_not_obstructed writer_iterations=508 initialized=17668 uninitialized=93533211
2024-09-15T18:10:32.655844Z : single_writer_is_not_obstructed writer_iterations=611 initialized=18388 uninitialized=53378768
ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.22s
```


#### Запуск теста [`ku/tests/1-time-2-once-lock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-2-once-lock.rs) под [Miri](https://github.com/rust-lang/miri)

```console
/.../nikka$ (cd ku; cargo clean; MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test 1-time-2-once-lock)
...
running 5 tests
test multiple_writers ... 2024-09-15T18:12:55.526813Z : multiple_writers iteration=0 successfully_initialized=1 initialized=267 uninitialized=1437
2024-09-15T18:12:57.337260Z : multiple_writers iteration=1 successfully_initialized=1 initialized=298 uninitialized=1642
2024-09-15T18:12:59.002762Z : multiple_writers iteration=2 successfully_initialized=1 initialized=230 uninitialized=2089
2024-09-15T18:13:00.723474Z : multiple_writers iteration=3 successfully_initialized=1 initialized=294 uninitialized=1534
2024-09-15T18:13:02.370623Z : multiple_writers iteration=4 successfully_initialized=1 initialized=232 uninitialized=1918
2024-09-15T18:13:04.138714Z : multiple_writers iteration=5 successfully_initialized=1 initialized=283 uninitialized=1844
2024-09-15T18:13:05.921776Z : multiple_writers iteration=6 successfully_initialized=1 initialized=266 uninitialized=2139
2024-09-15T18:13:07.717856Z : multiple_writers iteration=7 successfully_initialized=1 initialized=305 uninitialized=1560
2024-09-15T18:13:09.388395Z : multiple_writers iteration=8 successfully_initialized=1 initialized=265 uninitialized=1548
2024-09-15T18:13:11.051254Z : multiple_writers iteration=9 successfully_initialized=1 initialized=253 uninitialized=1659
ok
test set_get ... ok
test set_get_mut ... ok
test single_writer ... 2024-09-15T18:13:12.794088Z :    single_writer initialized=11 uninitialized=1039
ok
test single_writer_is_not_obstructed ... 2024-09-15T18:13:19.821631Z : single_writer_is_not_obstructed writer_iterations=118 initialized=357 uninitialized=23056
2024-09-15T18:13:26.292597Z : single_writer_is_not_obstructed writer_iterations=77 initialized=305 uninitialized=22597
2024-09-15T18:13:32.775937Z : single_writer_is_not_obstructed writer_iterations=82 initialized=314 uninitialized=22533
2024-09-15T18:13:38.895482Z : single_writer_is_not_obstructed writer_iterations=88 initialized=287 uninitialized=20771
2024-09-15T18:13:45.050933Z : single_writer_is_not_obstructed writer_iterations=78 initialized=310 uninitialized=20629
ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 53.85s
```


##### [Miri](https://github.com/rust-lang/miri) находит ошибки в использовании модели памяти

Эмулятор
[Miri](https://github.com/rust-lang/miri)
в том числе выполняет переупорядочивание инструкций в соответствии с моделью памяти языка:

> Miri supports almost all Rust language features; in particular, unwinding and concurrency are properly supported (including some experimental emulation of weak memory effects, i.e., reads can return outdated values).

За счёт этого можно найти ошибки в расстановке
[`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html).
Поэтому, после внесения ошибки в
[`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html)
под
[Miri](https://github.com/rust-lang/miri)
тест падает:

```console
$ (cd ku; cargo clean; MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test 1-time-2-once-lock)
     Removed 857 files, 443.6MiB total
   Compiling autocfg v1.3.0
   Compiling proc-macro2 v1.0.86
   Compiling unicode-ident v1.0.12
   ...
   Compiling ku v0.1.0 (/.../nikka/ku)
   Compiling nix v0.27.1
   Compiling rand v0.8.5
   Compiling rstest v0.22.0
    Finished `test` profile [unoptimized + debuginfo] target(s) in 49.68s
     Running tests/1-time-2-once-lock.rs (/.../nikka/target/miri/x86_64-unknown-linux-gnu/debug/deps/1_time_2_once_lock-81e61ffec42dd2af)

running 5 tests
test multiple_writers ... error: Undefined Behavior: Data race detected between (1) retag write on thread `writer #1` and (2) retag read of type `std::mem::MaybeUninit<[usize; 4]>` on thread `reader #1` at alloc261701+0x10. (2) just happened here
   --> /.../nikka/ku/src/sync/once_lock.rs:49:27
    |
49  |             Some(unsafe { (*self.data.get()).assume_init_ref() })
    |                           ^^^^^^^^^^^^^^^^^^ Data race detected between (1) retag write on thread `writer #1` and (2) retag read of type `std::mem::MaybeUninit<[usize; 4]>` on thread `reader #1` at alloc261701+0x10. (2) just happened here
    |
help: and (1) occurred earlier here
   --> /.../nikka/ku/src/sync/once_lock.rs:86:13
    |
86  |             data.write(value);
    |             ^^^^^^^^^^^^^^^^^
    = help: retags occur on all (re)borrows and as well as when references are copied or moved
    = help: retags permit optimizations that insert speculative reads or writes
    = help: therefore from the perspective of data races, a retag has the same implications as a read or write
    = help: this indicates a bug in the program: it performed an invalid operation, and caused Undefined Behavior
    = help: see https://doc.rust-lang.org/nightly/reference/behavior-considered-undefined.html for further information
    = note: BACKTRACE (of the first span) on thread `reader #1`:
    = note: inside `ku::sync::OnceLock::<[usize; 4]>::get` at /.../nikka/ku/src/sync/once_lock.rs:49:27: 49:45
note: inside `reader`
   --> ku/tests/1-time-2-once-lock.rs:223:29
    |
223 |         if let Some(data) = once_lock.get() {
    |                             ^^^^^^^^^^^^^^^
note: inside closure
   --> ku/tests/1-time-2-once-lock.rs:169:36
    |
169 |                     .spawn(move || reader(&once_lock, &readers_run))
    |                                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

note: some details are omitted, run with `MIRIFLAGS=-Zmiri-backtrace=full` for a verbose backtrace

error: aborting due to 1 previous error

error: test failed, to rerun pass `--test 1-time-2-once-lock`

Caused by:
  process didn't exit successfully: `/.../.rustup/toolchains/nightly-2024-09-07-x86_64-unknown-linux-gnu/bin/cargo-miri runner /.../nikka/target/miri/x86_64-unknown-linux-gnu/debug/deps/1_time_2_once_lock-81e61ffec42dd2af` (exit status: 1)
note: test exited abnormally; to see the full output pass --nocapture to the harness.
```


#### Оптимизация [`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html) с помощью [Miri](https://github.com/rust-lang/miri)

Вы также можете попробовать расслабить проставленные вами
[`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html).
Если при этом
[Miri](https://github.com/rust-lang/miri)
не найдёт ошибок, это повод задуматься.
Возможно, тесты не полны или действительно более слабые
[`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html)
корректны.


### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/sync/once_lock.rs | 54 +++++++++++++++++++++++++++++++++++++++++-------------
 1 file changed, 41 insertions(+), 13 deletions(-)
```
