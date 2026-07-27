## Sequence lock

В этой задаче нужно реализовать
[Sequence lock](https://en.wikipedia.org/wiki/Seqlock).
Этот алгоритм синхронизации не требует захвата блокировки в читателях.
Поэтому писатели никогда их не ждут.
Другими словами, он отдаёт максимальное предпочтение писателям.
Вплоть до того, что при высокой активности писателей читатели работать не смогут.


### Модель памяти, Acquire и Release, опасность SeqCst

К сожалению, как и в предыдущей задаче,
на архитектуре
[x86-64](https://en.wikipedia.org/wiki/X86-64)
тест не может проверить корректность расстановки
[`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html).
Поэтому ей стоит уделить повышенное внимание.
[Как и раньше](../../lab/book/1-time-3-spinlock.html#%D0%9C%D0%BE%D0%B4%D0%B5%D0%BB%D1%8C-%D0%BF%D0%B0%D0%BC%D1%8F%D1%82%D0%B8-acquire-%D0%B8-release),
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
использовать не рекомендуется.


### Основные инварианты Sequence lock

[Sequence lock](https://en.wikipedia.org/wiki/Seqlock) хранит счётчик `sequence`.
При операциях захвата блокировки он последовательно возрастает, что и дало название этому примитиву синхронизации.
Поддерживается такой инвариант:

- Чётное значение `sequence` означает, что [sequence lock](https://en.wikipedia.org/wiki/Seqlock) не заблокирован.
- Нечётное значение `sequence` означает, что [sequence lock](https://en.wikipedia.org/wiki/Seqlock) захвачен на запись.
- При захвате на запись и при освобождении `sequence` инкрементируется.
- Захват на чтение не поддерживается, а читатели никак не меняют значение `sequence`.


### Алгоритм работы писателя

При захвате
[sequence lock](https://en.wikipedia.org/wiki/Seqlock)
на запись, писатель должен прежде всего дождаться освобождения блокировки, если она уже захвачена конкурирующим писателем.
Как только `sequence` стал чётен, писатель инкрементирует его.
При этом проверка чётности и инкремент должны составлять атомарную операцию.
После этого писатель владеет блокировкой и может модифицировать защищаемые данные.
Для того чтобы освободить блокировку, писатель инкрементирует `sequence` ещё раз,
так что `sequence` опять становится чётным.


### Алгоритм работы читателя

Читатель должен дождаться пока
[sequence lock](https://en.wikipedia.org/wiki/Seqlock)
не окажется в свободном состоянии.
То есть, момента когда `sequence` станет чётным.
После этого читатель копирует защищаемые данные в отдельное место в памяти.
Во время такого копирования
[sequence lock](https://en.wikipedia.org/wiki/Seqlock)
мог быть заблокирован на запись и защищаемые данные могли быть модифицированы конкурентно.
Поэтому по окончании копирования читатель проверяет, что `sequence` не изменился.
Если `sequence` не изменился, то конкурентных записей не было и можно пользоваться скопированными данными.
Иначе, была конкурентная запись, поэтому скопированные данные могут быть не консистентны.
В этом случае они отбрасываются и процедура чтения повторяется с самого начала --- ожидания освобождения
[sequence lock](https://en.wikipedia.org/wiki/Seqlock).
При этом мы считаем, что между двумя проверками счётчик `sequence`
не успеет переполниться и достичь первоначального значения.


### Приём read-don't-modify-write

С точки зрения расстановки
[`core::sync::atomic::Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html)
при работе с `sequence`,
и читатель и писатель должны:

- Сначала его "захватить" --- [`Ordering::Acquire`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Acquire).
- Поработать с защищаемыми данными.
- "Освободить" --- [`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release).

При этом
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release)
[можно использовать только в операции записи](../../lab/book/1-time-3-spinlock.html#%D0%9C%D0%BE%D0%B4%D0%B5%D0%BB%D1%8C-%D0%BF%D0%B0%D0%BC%D1%8F%D1%82%D0%B8-acquire-%D0%B8-release).
Так как читатель никак не модифицирует `sequence`, кажется что
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release)
он использовать не может.
Что сломало бы синхронизацию.

На помощь приходит
[приём read-don't-modify-write](https://www.hpl.hp.com/techreports/2012/HPL-2012-68.pdf),
идея которого состоит в следующем.
Нам нужно подобрать операцию записи, которая не изменит записываемое значение.
Эта операция, естественно, должна знать какое значение было раньше, поэтому нам подойдут только операции,
которые выполняют и чтение и запись.
Например, `fetch_add(0)` атомарно прочитает значение, добавит к нему `0` и запишет
не изменившееся значение обратно.

Обратите внимание, что использование
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
не поможет избавиться от необходимости в приёме read-don't-modify-write.

> Даже если обновлённое значение не отличается от исходного,
> по [протоколу когерентности кешей](https://en.wikipedia.org/wiki/Cache_coherence)
> кеш-линия должна быть захвачена яром процессора в эксклюзивное владение.
> То есть, читатели захватывают кеш линию эксклюзивно.
> Что при высокой нагрузке на
> [sequence lock](https://en.wikipedia.org/wiki/Seqlock)
> приводит к конкуренции за неё между писателями и читателями.
> Другими словами, читатели всё же мешают писателям.
> Хотя логически и не захватывают
> [sequence lock](https://en.wikipedia.org/wiki/Seqlock).

> Эту задачу можно рассматривать как подготовку к следующей.
> В которой нам, к сожалению, уже нельзя будет воспользоваться приёмом read-don't-modify-write.


### Ссылки

- [Writing a seqlock in Rust.](https://pitdicker.github.io/Writing-a-seqlock-in-Rust/)
- [Can Seqlocks Get Along With Programming Language Memory Models?](https://www.hpl.hp.com/techreports/2012/HPL-2012-68.pdf)
- [Crate seqlock.](https://docs.rs/seqlock/0.1.2/seqlock/)


### Структура [`SequenceLockGuard`](../../doc/ku/sync/sequence_lock/struct.SequenceLockGuard.html)

Структура
[`SequenceLockGuard`](../../doc/ku/sync/sequence_lock/struct.SequenceLockGuard.html)
по своему назначению и реализации полностью аналогична структуре
[`SpinlockGuard`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html).
Посмотрите про неё
[в предыдущей задаче](../../lab/book/1-time-3-spinlock.html#%D0%A1%D1%82%D1%80%D1%83%D0%BA%D1%82%D1%83%D1%80%D0%B0-spinlockguard),
даже если не решали её.


### Задача 3 --- реализация [`SequenceLock`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html)

Реализуйте
[метод](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html#method.write_lock)

```rust
fn SequenceLock::write_lock(&self) -> SequenceLockGuard<'_, T>
```
в файле
[`ku/src/sync/sequence_lock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/sequence_lock.rs).

Он должен захватить блокировку на запись.
И похож на методы
[`Spinlock::lock()`](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.lock)
и
[`Spinlock::try_lock_impl()`](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.try_lock_impl)
из
[предыдущей задачи](../../lab/book/1-time-3-spinlock.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-2--%D1%80%D0%B5%D0%B0%D0%BB%D0%B8%D0%B7%D0%B0%D1%86%D0%B8%D1%8F-spinlock).

Реализуйте
[метод](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html#method.write)

```rust
unsafe fn SequenceLock::write(&self) -> SequenceLockGuard<'_, T>
```
в файле
[`ku/src/sync/sequence_lock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/sequence_lock.rs).

В отличие от метода
[`SequenceLock::write_lock()`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html#method.write_lock)
он считает, что конкурирующих писателей нет.
Это возможно в случае, когда писатель только один.
Или в случае, когда писатели синхронизируются между собой другим примитивом синхронизации.
При сохранении в
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)
информации о захватившем его месте в коде, у нас как раз реализуется второй вариант ---
писатели уже синхронизированы через сам этот
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html).
Итак, конкурирующих писателей нет, а читателей писатели в любом случае не ждут.
Значит, в этом методе не нужен цикл активного ожидания.
А сам метод проще чем
[`SequenceLock::write_lock()`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html#method.write_lock).
Но
[`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html)
в нём тоже нужно расставить правильно.
Сам метод прост ценой повышения сложности его использования.
Использующий код должен гарантировать отсутствие конкурирующих писателей.
Он может содержать ошибку, поэтому полезно запаниковать, если метод
[`SequenceLock::write()`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html#method.write)
заметит по значению поля
[`SequenceLock::sequence`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html#structfield.sequence),
что уже захвачена блокировка на запись.
Для этого подойдёт макрос
[`core::panic!()`](https://doc.rust-lang.org/nightly/core/macro.panic.html).
По той же причине метод
[`SequenceLock::write()`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html#method.write)
помечен как `unsafe`.
Это символизирует, что для выполнения гарантий
[`SequenceLock`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html),
в том числе эксклюзивности изменяемых ссылок на защищаемые данные,
он требует определённых гарантий от вызывающего его кода.


Реализуйте
[метод](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html#method.read)

```rust
fn SequenceLock::read(&self) -> T
```
в файле
[`ku/src/sync/sequence_lock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/sequence_lock.rs).

Для того чтобы прочитать защищаемые данные, используйте функцию
[`core::ptr::read_volatile()`](https://doc.rust-lang.org/nightly/core/ptr/fn.read_volatile.html):

```rust
let data = unsafe { ptr::read_volatile(self.data.get()) };
```

Реализуйте
[метод](../../doc/ku/sync/sequence_lock/struct.SequenceLockGuard.html#method.drop)

```rust
fn SequenceLockGuard::drop(&mut self)
```

в файле
[`ku/src/sync/sequence_lock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/sequence_lock.rs).
Он должен освободить захваченную блокировку на запись.

Вам могут пригодиться:

- Метод [`core::sync::atomic::AtomicBool::compare_exchange()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.compare_exchange).
- Метод [`core::sync::atomic::AtomicBool::compare_exchange_weak()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak).
- Метод [`core::sync::atomic::AtomicBool::load()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.load).
- Метод [`core::sync::atomic::AtomicBool::store()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.store).
- Метод [`core::sync::atomic::AtomicUsize::fetch_add()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicUsize.html#method.fetch_add).
- Функция [`core::ptr::read_volatile()`](https://doc.rust-lang.org/nightly/core/ptr/fn.read_volatile.html).
- Функция [`core::hint::spin_loop()`](https://doc.rust-lang.org/nightly/core/hint/fn.spin_loop.html), которая сообщает процессору, что он находится в цикле активного ожидания внешнего события.
- Метод [`SequenceLock::is_locked(sequence: u64)`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html#method.is_locked). Он возвращает `true`, если значение `sequence` означает, что захвачена блокировка на запись.


### Доработайте [`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)

Если вы решали предыдущую задачу, то остаётся только доработать её с помощью
[`SequenceLock`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html).

В
[методе](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.try_lock_impl)

```rust
fn Spinlock::try_lock_impl(
    &self,
    max_tries: usize,
    callsite: Callsite,
) -> Option<SpinlockGuard<'_, T>>
```

в файле [`ku/src/sync/spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/spinlock.rs)
сохраните аргумент `callsite` в поле
[`Spinlock::owner`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.owner),
если
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)
удалось захватить.
Так как это единственное место, где оно будет записываться, и оно уже синхронизировано самим
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html),
будет достаточно метода
[`SequenceLock::write()`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html#method.write).

А в
[методе](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.lock)

```rust
fn Spinlock::lock(&self) -> SpinlockGuard<'_, T>
```

в файле
[`ku/src/sync/spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/spinlock.rs)
добавьте в сообщение паники данные о владельце
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html),
прочитав их из поля
[`Spinlock::owner`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.owner).
Для удобства также стоит добавить информацию о строке кода, где была определена переменная типа
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html),
на которой возникла взаимоблокировка.
И о строке кода, которая пытаясь захватить это
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html),
создала взаимоблокировку, вместе с ведущим к ней backtrace.
Теперь тесты ожидают, что в сообщении паники об обнаружении взаимоблокировки присутствуют
ссылки на эти строки кода, а также подстрока `"deadlock"`.

Аналогично доработайте реализацию типажа
[`core::fmt::Debug`](https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html)
для
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)
в файле
[`ku/src/sync/spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/spinlock.rs).
Если спин–блокировка захвачена, --- распечатайте кто ей сейчас владеет.

Теперь сообщения об обнаружении взаимоблокировки будут содержать полезные для отладки данные.
Например:
```console
...
     Running tests/1-time-2-spinlock.rs (.../nikka/target/debug/deps/1_time_2_spinlock-ab8d2daeff2439d2)
...
thread '<unnamed>' panicked at 'deadlock detected on spinlock defined at ku/tests/1-time-2-spinlock.rs:85:20, locked at ku/tests/1-time-2-spinlock.rs:86:25, backtrace: [0x4332DF 0x423B65 0x42FA55 0x411205 0x421A85 0x408F87 0x40943D 0x408B22 0x41BD55 0x40F845 0x41DA71 0x50F3D3], lock attempt at ku/tests/1-time-2-spinlock.rs:89:19, backtrace: [0x433639 0x423B65 0x42FA55 0x411205 0x421A85 0x408F87 0x40943D 0x408B22 0x41BD55 0x40F845 0x41DA71 0x50F3D3]', ku/tests/1-time-2-spinlock.rs:89:19
```
С помощью `llvm-symbolizer` можно расшифровать backtrace:
```console
$ echo '0x4332DF 0x423B65 0x42FA55 0x411205 0x421A85 0x408F87 0x40943D 0x408B22 0x41BD55 0x40F845 0x41DA71 0x50F3D3' | tr -d ',_' | tr 'v ' 'x\n' | llvm-symbolizer --exe .../nikka/target/debug/deps/1_time_2_spinlock-ab8d2daeff2439d2
_1_time_2_spinlock::recursive_deadlock::ntest_callback::h5b95218d6b1ff806
.../nikka/ku/tests/1-time-2-spinlock.rs:86:16

_1_time_2_spinlock::recursive_deadlock::_$u7b$$u7b$closure$u7d$$u7d$::he690e3d8ee620dc1
.../nikka/ku/tests/1-time-2-spinlock.rs:81:1
...
```
Аргумент для `--exe`, то есть путь к запускаемому бинарнику, можно взять из сообщения `cargo test`:
```
     Running tests/1-time-2-spinlock.rs (.../nikka/target/debug/deps/1_time_2_spinlock-ab8d2daeff2439d2)
```


> ### Гонка данных
>
> В нашей реализации
> [`SequenceLock`](../../doc/ku/sync/sequence_lock/struct.SequenceLock.html)
> есть гонка данных, которая формально является
> [Undefined Behavior](https://doc.rust-lang.org/nomicon/races.html).
> Причём [Miri](https://github.com/rust-lang/miri) её находит:
> ```console
> /.../nikka/ku$ cargo miri test --test 1-time-3-sequence-lock
> Preparing a sysroot for Miri (target: x86_64-unknown-linux-gnu)... done
>     Finished test [unoptimized + debuginfo] target(s) in 0.06s
>      Running tests/1-time-3-sequence-lock.rs (/.../nikka/target/miri/x86_64-unknown-linux-gnu/debug/deps/1_time_3_sequence_lock-091aeae8a41b580f)
>
> running 6 tests
> test exclusive_access ... ok
> test multiple_exclusive_writers ... ok
> test multiple_writers ... error: Undefined Behavior: Data race detected between (1) Read on thread `reader_thread #` and (2) Write on thread `writer_thread #` at alloc8. (2) just happened here
>    --> /.../nikka/ku/src/sync/sequence_lock.rs:197:18
>     |
> 197 |         unsafe { &mut *self.sequence_lock.data.get() }
>     |                  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Data race detected between (1) Read on thread `reader_thread #` and (2) Write on thread `writer_thread #` at alloc8. (2) just happened here
>     |
> help: and (1) occurred earlier here
>    --> /.../nikka/ku/src/sync/sequence_lock.rs:133:29
>     |
> 133 |         let data = unsafe { ptr::read_volatile(self.data.get()) };
>     |                             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
>     = help: this indicates a bug in the program: it performed an invalid operation, and caused Undefined Behavior
>     = help: see https://doc.rust-lang.org/nightly/reference/behavior-considered-undefined.html for further information
>     = note: BACKTRACE (of the first span):
>     = note: inside `<ku::sync::sequence_lock::SequenceLockGuard<'_, (usize, usize)> as std::ops::DerefMut>::deref_mut` at /.../nikka/ku/src/sync/sequence_lock.rs:197:18: 197:53
> note: inside `non_exclusive_writer`
>    --> ku/tests/1-time-3-sequence-lock.rs:275:9
>     |
> 275 |         lock.0 = i + 1;
>     |         ^^^^^^
> note: inside closure
>    --> ku/tests/1-time-3-sequence-lock.rs:149:21
>     |
> 149 |                     non_exclusive_writer(&SEQUENCE_LOCK, &RUN);
>     |                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
>
> note: some details are omitted, run with `MIRIFLAGS=-Zmiri-backtrace=full` for a verbose backtrace
>
> error: aborting due to previous error
>
> error: test failed, to rerun pass `--test 1-time-3-sequence-lock`
>
> Caused by:
>   process didn't exit successfully: `/home/sergey/.rustup/toolchains/nightly-2023-09-16-x86_64-unknown-linux-gnu/bin/cargo-miri runner /.../nikka/target/miri/x86_64-unknown-linux-gnu/debug/deps/1_time_3_sequence_lock-091aeae8a41b580f` (exit status: 1)
> note: test exited abnormally; to see the full output pass --nocapture to the harness.
> ```
>
> Пока предлагается оставить всё как есть, так как тут мы сталкиваемся с текущими органичениями языка.
>
> В
> [следующей задаче](../../lab/book/1-time-5-correlation-point.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-4--%D1%80%D0%B5%D0%B0%D0%BB%D0%B8%D0%B7%D0%B0%D1%86%D0%B8%D1%8F-atomiccorrelationpoint)
> на секвентную блокировку в частном случае, уже гонки данных не должно быть:
> ```console
> /.../nikka/ku$ cargo miri test --test 1-time-4-correlation-point
> Preparing a sysroot for Miri (target: x86_64-unknown-linux-gnu)... done
>    Compiling ku v0.1.0 (/.../nikka/ku)
>     Finished test [unoptimized + debuginfo] target(s) in 0.49s
>      Running tests/1-time-4-correlation-point.rs (/.../nikka/target/miri/x86_64-unknown-linux-gnu/debug/deps/1_time_4_correlation_point-a4a526eb7ffb5ab4)
>
> running 3 tests
> test atomic_correlation_point ... ok
> test correlation_point ... ok
> test single_writer ... ok
>
> test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
> ```


### Проверьте себя

На этот раз тесты находятся не в `kernel`, а в `ku` ---
[`ku/tests/1-time-3-sequence-lock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-3-sequence-lock.rs)
и
[`ku/tests/1-time-3-deadlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-3-deadlock.rs).
Их можно запустить командой `cargo test --test 1-time-3-sequence-lock --test 1-time-3-deadlock` в директории `ku` репозитория.
Вы увидите сборку и логи запуска тестов:

```console
/.../nikka$ (cd ku; cargo test --test 1-time-3-sequence-lock --test 1-time-3-deadlock)
   Compiling ku v0.1.0 (/.../nikka/ku)
    Finished test [unoptimized + debuginfo] target(s) in 2.25s
     Running tests/1-time-3-deadlock.rs (/.../nikka/target/debug/deps/1_time_3_deadlock-324948b448ed2776)

running 4 tests
2023-07-22T17:14:51.265702Z : deadlock_line_number_in_panic_message attempting to lock a locked spinlock spinlock=Spinlock { defined: ku/tests/1-time-3-deadlock.rs:42:20, locked: true, owner: ku/tests/1-time-3-deadlock.rs:43:25, backtrace: [0x40EE5E], stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2023-07-22T17:14:51.265704Z :      owner_line_number_in_panic_message attempting to lock a locked spinlock spinlock=Spinlock { defined: ku/tests/1-time-3-deadlock.rs:42:20, locked: true, owner: ku/tests/1-time-3-deadlock.rs:43:25, backtrace: [0x40EE5E], stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2023-07-22T17:14:51.265702Z : definition_line_number_in_panic_message attempting to lock a locked spinlock spinlock=Spinlock { defined: ku/tests/1-time-3-deadlock.rs:42:20, locked: true, owner: ku/tests/1-time-3-deadlock.rs:43:25, backtrace: [0x40EE5E], stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2023-07-22T17:14:51.265703Z :               deadlock_in_panic_message attempting to lock a locked spinlock spinlock=Spinlock { defined: ku/tests/1-time-3-deadlock.rs:42:20, locked: true, owner: ku/tests/1-time-3-deadlock.rs:43:25, backtrace: [0x40EE5E], stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2023-07-22T17:14:51.287957Z :      owner_line_number_in_panic_message dropping spinlock=ku/tests/1-time-3-deadlock.rs:42:20 stats=Stats { failures: 2, locks: 1, unlocks: 1, waits: 1000001 }
test owner_line_number_in_panic_message - should panic ... ok
2023-07-22T17:14:51.288244Z :               deadlock_in_panic_message dropping spinlock=ku/tests/1-time-3-deadlock.rs:42:20 stats=Stats { failures: 2, locks: 1, unlocks: 1, waits: 1000001 }
test deadlock_in_panic_message - should panic ... ok
2023-07-22T17:14:51.289283Z :   deadlock_line_number_in_panic_message dropping spinlock=ku/tests/1-time-3-deadlock.rs:42:20 stats=Stats { failures: 2, locks: 1, unlocks: 1, waits: 1000001 }
test deadlock_line_number_in_panic_message - should panic ... ok
2023-07-22T17:14:51.293327Z : definition_line_number_in_panic_message dropping spinlock=ku/tests/1-time-3-deadlock.rs:42:20 stats=Stats { failures: 2, locks: 1, unlocks: 1, waits: 1000001 }
test definition_line_number_in_panic_message - should panic ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/1-time-3-sequence-lock.rs (/.../nikka/target/debug/deps/1_time_3_sequence_lock-4232a645c57babf4)

running 6 tests
test exclusive_access ... ok
test write ... ok
test write_lock ... ok
2023-07-22T17:14:52.298462Z : ThreadId(07) non_exclusivity_detection_count=9
test multiple_exclusive_writers ... ok
2023-07-22T17:14:54.712710Z : ThreadId(08) consistent_count=1007 inconsistent_count=0
test multiple_writers ... ok
2023-07-22T17:14:55.062643Z : ThreadId(09) consistent_count=17323 inconsistent_count=0
test single_writer ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.77s
```


### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/sync/sequence_lock.rs | 76 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++----------
 ku/src/sync/spinlock.rs      |  8 +++++---
 2 files changed, 71 insertions(+), 13 deletions(-)
```
