## Блокировки

Целью этого цикла задач является реализация
[блокировки с использованием цикла активного ожидания](https://en.wikipedia.org/wiki/Spinlock).
Мы предполагаем, что критические секции, пользующиеся этой блокировкой, будут короткими.
Если не делать эти задачи, то будет использоваться реализация
[`spin::mutex::Mutex`](../../doc/spin/mutex/struct.Mutex.html)
из сторонней библиотеки
[`spin`](../../doc/spin/index.html).
Она не предоставляет удобных средств отладки
[взаимоблокировок](https://en.wikipedia.org/wiki/Deadlock),
которую мы как раз и хотим реализовать.
Если вы пропустите эти задачи, а в будущем столкнётесь с зависаниями,
то настоятельно рекомендуется вернуться к этим задачам и всё же сделать их.
Тогда отлаживать зависания будет проще.

Честные алгоритмы обнаружения
[взаимоблокировок](https://en.wikipedia.org/wiki/Deadlock)
строят двудольный граф на потоках управления и примитивах синхронизации.
В нём проводятся направленные рёбра из вершин, отвечающих каждому заблокированному
примитиву синхронизации в вершину владеющего им в данный момент потоку управления.
А также направленные рёбра из вершин, отвечающих каждому ожидающему потоку управления в вершину
примитива синхронизации, освобождения которого ждёт этот поток.
Цикл в таком графе и есть
[взаимоблокировка](https://en.wikipedia.org/wiki/Deadlock).
Было бы удобно иметь столь полную отладочную информацию при возникновении
[взаимоблокировок](https://en.wikipedia.org/wiki/Deadlock).
Но это потребует достаточно сложной реализации.

Пойдём более простым путём.
Будем считать, что если в течение длительного времени потоку не удалось
захватить спин–блокировку, то возникла
[взаимоблокировока](https://en.wikipedia.org/wiki/Deadlock).
Причём в ней, кроме ожидающего потока участвует также текущий владелец
спин–блокировки.
То есть, достаточно будет:

- Ограничить ожидание освобождения блокировки при попытке захвата.
- Сохранять в спин–блокировке информацию о захватившем её месте кода.

Первый пункт реализуем в этой задаче, вместе с основным функционалом спин–блокировки.
А со вторым пунктом нам поможет следующая задача.


### Модель памяти, Acquire и Release, опасность SeqCst

Рекомендуется освоить
[`Ordering::Acquire`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Acquire)
и
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release).
Тем более, что у них очень интуитивная семантика.
А именно,
[`Ordering::Acquire`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Acquire)
должен стоять на операции захвата примитива синхронизации,
а
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release) ---
на его освобождении.
Обратите внимание, что они должны стоять на одной и той же атомарной переменной.

Но нужно держать в голове, что
[`Ordering::Acquire`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Acquire)
можно ставить только на операции, выполняющие чтение.
И она относится именно к читающей части операции, если сама операция выполняет кроме чтения ещё и запись.
А
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release)
можно ставить только на операции, выполняющие запись.
Аналогично,
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release)
относится именно к записывающей части операции, если сама операция выполняет кроме записи ещё и чтение.
На операции выполняющие и чтение и запись, можно также ставить
[`Ordering::AcqRel`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.AcqRel),
это будет означать что на читающую часть распространяется
[`Ordering::Acquire`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Acquire),
а на пишущую ---
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release).

То есть, например, никаким образом на операцию чтения нельзя поставить
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release).
Мы столкнёмся с этим ограничением в следующей задаче, и будем вынуждены ради необходимого
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release)
при освобождении примитива синхронизации сделать запись, которая не нужна ни для чего больше.

Использование
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
не снимает такое ограничение.
На операциях, выполняющих только чтение, она почти что превращается в
[`Ordering::Acquire`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Acquire),
а на операциях, выполняющих только запись --- в
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release).

Обратите внимание, что
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
использовать не рекомендуется.
Так как мнение "использовать
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
гарантированно безопасно" неверно.
Режим
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
позволяет делать довольно специфические вещи, которые реально нужны довольно редко.
[А вот гарантий корректности не даёт, это заблуждение](https://github.com/rust-lang/nomicon/issues/166).

Если в алгоритме синхронизации вам понадобилось написать что-нибудь вроде

```rust
... sequence.load(Ordering::Release) ...
```

Компилятор выдаст ошибку:

```console
$ cargo test --test 1-time-3-sequence-lock
   Compiling ku v0.1.0 (.../nikka/ku)
error: atomic loads cannot have `Release` or `AcqRel` ordering
   --> ku/src/sync/sequence_lock.rs:139:48
    |
139 |         ... self.sequence.load(Ordering::Release) ...
    |                                ^^^^^^^^^^^^^^^^^
    |
    = note: `#[deny(invalid_atomic_ordering)]` on by default
    = help: consider using ordering modes `Acquire`, `SeqCst` or `Relaxed`

error: could not compile `ku` due to previous error
```

А если её подавить с помощью `#[allow(invalid_atomic_ordering)]`, то уже при запуске будет
[паника](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicI64.html#panics):

```
thread '<unnamed>' panicked at 'there is no such thing as a release load', /rustc/bc4b39c271bbd36736cbf1c0a1ac23d5df38d365/library/core/src/sync/atomic.rs:2964:24
```

Если же в этом месте поменять
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release)
на
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst),
не меняя остальную часть алгоритма синхронизации,
то компиляция пройдёт и паники не будет.
Но код скорее всего будет **некорректен**, --- простая замена на
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
не исправит сам алгоритм синхронизации.
И это будет почти что эквивалентно записи
```
... sequence.load(Ordering::Acquire) ...
```
Плюс дополнительное снижающее эффективность свойство, --- все атомарные операции, помеченные
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst),
будут выстроены в одинаковый с точки зрения всех ядер порядок.

То есть, использование
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
скроет ошибку в алгоритме синхронизации, о наличии которой заботливо предупреждали компилятор и паника.
К такому же эффекту приведёт смена
[`Ordering::Release`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Release)
на
[`Ordering::Relaxed`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Relaxed).
Но [`Ordering::Relaxed`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.Relaxed)
хотя бы бросается в глаза в коде.
В этой задаче вы скорее всего не напишете код, в котором
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
скрыл бы наличие ошибки.
Но вот в следующей --- это весьма вероятно.
А рекомендуется рассматривать эту задачу как подготовку к следующим двум.

> Дополнительные ссылки про модель памяти:
>
> - [Модель памяти Rust](https://doc.rust-lang.org/nomicon/atomics.html#atomics).
> - Цикл статей Джефа Прешинга:
>   - [Atomic vs. Non-Atomic Operations](https://preshing.com/20130618/atomic-vs-non-atomic-operations/).
>   - [The Happens-Before Relation](https://preshing.com/20130702/the-happens-before-relation/).
>   - [The Synchronizes-With Relation](https://preshing.com/20130823/the-synchronizes-with-relation/).
>   - ...
>   - [Acquire and Release Semantics](https://preshing.com/20120913/acquire-and-release-semantics/).
>   - ...
> - [Common Compiler Optimisations are Invalid in the C11 Memory Model and what we can do about it](https://plv.mpi-sws.org/c11comp/popl15.pdf).
> - [Теория и практика многопоточной синхронизации (ТПМС, Concurrency) / Лекции / Весна 2022 / Роман Липовский](https://youtu.be/playlist?list=PL4_hYwCyhAva37lNnoMuBcKRELso5nvBm):
>   - [ТПМС / Лекция 7 / Shared Memory: Memory Models](https://youtu.be/CEzFMP78D1Y).
> - [Теория и практика многопоточной синхронизации (ТПМС, Concurrency) / Семинары / Весна 2022 / Роман Липовский](https://youtu.be/playlist?list=PL4_hYwCyhAvYTxm55RBm_HA5Bq5W1Nv-R):
>   - [Concurrecy(семинар) 7. Модели памяти](https://youtu.be/sU7jguD_lqI).
>   - [Concurrecy(семинар) 8. Модели памяти. Продолжение](https://youtu.be/Ys5-RcB1iSE).


### Theory and Practice of Concurrency

Более подробно познакомиться с конкурентностью, атомарными переменными, синхронизациями, моделями памяти и т.п.,
вы можете на курсе Ромы Липовского "Theory and Practice of Concurrency":

- Публичный доступ:
  - [Плейлист с лекциями](https://youtube.com/playlist?list=PL4_hYwCyhAva37lNnoMuBcKRELso5nvBm).
  - [Репозиторий курса](https://gitlab.com/Lipovsky/concurrency-course).
- Личный кабинет ШАД, [весна 2023](https://lk.yandexdataschool.ru/courses/2023-spring/7.1141-theory-and-practice-of-concurrency/classes/) или [весна 2022](https://lk.yandexdataschool.ru/courses/2022-spring/7.1013-theory-and-practice-of-concurrency/classes/).
- [Wiki yandex-team](https://wiki.yandex-team.ru/hr/volnoslushateli/lekcii-shad/).


### Запуск тестов под [Miri](https://github.com/rust-lang/miri)

Тесты блокировок мы будем запускать в том числе под эмулятором
[Miri](https://github.com/rust-lang/miri)
промежуточного представления
[Mid-level Intermediate Representation (MIR)](https://rustc-dev-guide.rust-lang.org/mir/index.html)
для Rust'овского кода.


#### Установка [Miri](https://github.com/rust-lang/miri)

Сначала нужно будет поставить
[Miri](https://github.com/rust-lang/miri),
выполнив команды

```console
/.../nikka$ rustup component add miri
info: syncing channel updates for 'nightly-2023-09-16-x86_64-unknown-linux-gnu'
info: latest update on 2023-09-16, rust version 1.74.0-nightly (20999de3a 2023-09-15)
info: component 'clippy' for target 'x86_64-unknown-linux-gnu' is up to date
info: component 'llvm-tools' for target 'x86_64-unknown-linux-gnu' is up to date
info: component 'rust-src' is up to date
info: component 'rustc-dev' for target 'x86_64-unknown-linux-gnu' is up to date
info: component 'rustfmt' for target 'x86_64-unknown-linux-gnu' is up to date
info: downloading component 'miri'
info: installing component 'miri'
info: component 'miri' for target 'x86_64-unknown-linux-gnu' is up to date
```

и

```console
/.../nikka$ cargo miri setup
Preparing a sysroot for Miri (target: x86_64-unknown-linux-gnu)...
A sysroot for Miri is now available in `/.../.cache/miri`.
```

из корня репозитория.
Если вы запустите `cargo miri setup` не из корня репозитория, то можете получить ошибку вроде такой:

```console
error[E0464]: multiple candidates for `rmeta` dependency `core` found
 --> /.../.rustup/toolchains/nightly-2023-09-16-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/rustc-std-workspace-core/lib.rs:4:9
  |
4 | pub use core::*;
  |         ^^^^
  |
  = note: candidate #1: /tmp/.tmp7Ev1h4/target/x86_64-unknown-linux-gnu/release/deps/libcore-e0e4ecdb825dac0d.rmeta
  = note: candidate #2: /tmp/.tmp7Ev1h4/target/x86_64-unknown-linux-gnu/release/deps/libcore-8c8751fb386a7e4e.rmeta

For more information about this error, try `rustc --explain E0464`.
error: could not compile `rustc-std-workspace-core` (lib) due to previous error
fatal error: failed to build sysroot: sysroot build failed
```


#### Пример запуска теста под [Miri](https://github.com/rust-lang/miri)

Запустить тест под
[Miri](https://github.com/rust-lang/miri)
можно с помощью команды `cargo miri test`,
дополнительно передав ей параметр окружения `MIRIFLAGS="-Zmiri-disable-isolation"`.

```console
/.../nikka$ (cd ku; MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test 1-time-4-correlation-point)
Preparing a sysroot for Miri (target: x86_64-unknown-linux-gnu)... done
...
running 3 tests
test atomic_correlation_point ... ok
test correlation_point ... ok
test single_writer ... 2023-11-05T13:24:36.597550Z : ThreadId(07) consistent_count=19458 inconsistent_count=0
ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```


#### [Miri](https://github.com/rust-lang/miri) находит ошибки в использовании модели памяти

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
/.../nikka$ (cd ku; MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test 1-time-4-correlation-point)
Preparing a sysroot for Miri (target: x86_64-unknown-linux-gnu)... done
...
running 3 tests
test atomic_correlation_point ... ok
test correlation_point ... ok
test single_writer ... 2023-11-05T13:35:31.653085Z X reader_thread #6 inconsistent data data=CorrelationPoint { count: 1, tsc: 4 } iteration=1
2023-11-05T13:35:31.633290Z X reader_thread #7 inconsistent data data=CorrelationPoint { count: 7, tsc: 16 } iteration=0
2023-11-05T13:35:31.638295Z X reader_thread #0 inconsistent data data=CorrelationPoint { count: 61, tsc: 124 } iteration=0
2023-11-05T13:35:31.620411Z X reader_thread #9 inconsistent data data=CorrelationPoint { count: 7, tsc: 16 } iteration=0
2023-11-05T13:35:31.616408Z X reader_thread #3 inconsistent data data=CorrelationPoint { count: 7, tsc: 16 } iteration=0
2023-11-05T13:35:31.634590Z X reader_thread #8 inconsistent data data=CorrelationPoint { count: 1, tsc: 4 } iteration=0
2023-11-05T13:35:31.706226Z X reader_thread #4 inconsistent data data=CorrelationPoint { count: 100, tsc: 202 } iteration=1
2023-11-05T13:35:31.872129Z X reader_thread #2 inconsistent data data=CorrelationPoint { count: 175, tsc: 352 } iteration=2
2023-11-05T13:35:31.871054Z X reader_thread #1 inconsistent data data=CorrelationPoint { count: 172, tsc: 346 } iteration=4
2023-11-05T13:35:32.332227Z X reader_thread #5 inconsistent data data=CorrelationPoint { count: 349, tsc: 700 } iteration=9
2023-11-05T13:35:59.537162Z : ThreadId(07) consistent_count=14210 inconsistent_count=3793
FAILED

failures:

---- single_writer stdout ----
thread '<unnamed>' panicked at ku/tests/1-time-4-correlation-point.rs:94:5:
assertion `left == right` failed: detected 3793 inconsistent data reads
  left: 3793
 right: 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
thread 'single_writer' panicked at ku/tests/1-time-4-correlation-point.rs:49:1:
explicit panic


failures:
    single_writer

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

error: test failed, to rerun pass `--test 1-time-4-correlation-point`
```

Хотя, с той же ошибкой, при обычном запуске тест проходит:

```console
/.../nikka$ (cd ku; cargo test --test 1-time-4-correlation-point)
...
running 3 tests
test correlation_point ... ok
test atomic_correlation_point ... ok
2023-11-05T13:39:01.075616Z : ThreadId(07) consistent_count=3832656 inconsistent_count=0
test single_writer ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.42s
```
