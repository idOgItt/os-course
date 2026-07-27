## Счётчики тиков различных источников времени

Структура
[`ku::time::correlation_point::CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html)
из файла [`ku/src/time/correlation_point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/time/correlation_point.rs)
предназначена для привязки счётчика тактов процессора к другим часам в один и тот же момент времени.
Она содержит поля:

- [`CorrelationPoint::tsc`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.tsc) со значением счётчика тактов процессора.
- [`CorrelationPoint::count`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.count) со значением счётчика тиков другого источника времени в тот же момент.

Если значение
[`CorrelationPoint::tsc`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.tsc)
равно нулю, то структура в целом считается невалидной ---
[`CorrelationPoint::invalid()`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#method.invalid).
Это означает, что
[`CorrelationPoint::count`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.count)
не привязан ни к какому значению
[`CorrelationPoint::tsc`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.tsc).
И значение структуры относится не к самому тику, а к промежутку после тика
[`CorrelationPoint::count`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.count)
и до следующего.
При этом значение
[`CorrelationPoint::count`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.count)
может быть использовано как текущее время с низким разрешением --- частотой соответствующих часов.

Посмотрите код реализации структуры
[`ku::time::correlation_point::CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html),
он достаточно прост.

Более интересная структура
[`ku::time::correlation_point::AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
из файла [`ku/src/time/correlation_point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/time/correlation_point.rs) предназначена для конкурентного доступа к значениям
[`CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html).
То есть [`CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html) и
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
соотносятся также как примитивный тип
[`i64`](https://doc.rust-lang.org/nightly/core/primitive.i64.html)
и атомарный
[`core::sync::atomic::AtomicI64`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicI64.html).

Атомарность нужна для того, чтобы конкурентно

- в обработчике прерывания от часов обновлять счётчики [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html);
- а в обычном коде читать эти счётчики, чтобы "посмотреть на часы".

В [`CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html) два числа типа
[`i64`](https://doc.rust-lang.org/nightly/core/primitive.i64.html),
а в
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html) два соответствующих им
[`AtomicI64`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicI64.html).
Нам нужно читать и записывать эти два значения согласованно, поэтому и возникают небольшие сложности.

Можно было бы завести блокировку на доступ к полям
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html),
но это потребовало бы захватывать её из прерывания.
Что практически гарантированно приведёт к проблемам:

- Захват этой блокировки должен ещё и запрещать прерывания. Иначе возможна [взаимоблокировка](https://en.wikipedia.org/wiki/Deadlock), подумайте почему. А Nikka предпочитает не отключать прерывания на потенциально длительное время удержания какой-нибудь блокировки.
- Иногда прерывание будет ждать другой код, пока он не отпустит эту блокировку. Обычно прерывания являются более приоритетной деятельностью, поэтому возникнет нежелательная [инверсия приоритетов](https://en.wikipedia.org/wiki/Priority_inversion).
- Писать мы хотим из прерывания в режиме ядра. А читать, --- в том числе, из режима пользователя. При этом с одной стороны, ядро должно обеспечить консистентность чтения коду пользователя. А с другой --- оно не должно допустить чтобы злонамеренный код из пользовательского режима мог заблокировать ядро, в том числе навечно.

Вообще, захват блокировок из прерываний --- очень плохая идея.

Поэтому реализуем
[неблокирующую синхронизацию](https://en.wikipedia.org/wiki/Non-blocking_algorithm)
для согласованного доступа к полям
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html).
Которая будет отдавать предпочтение писателю, так как это обработчик прерывания в ядре.
Он всегда сможет завершить работу по обновлению структуры за фиксированное и достаточно небольшое количество тактов.
И никогда не будет ждать читателей, которые могут быть запущены как в режиме ядра, так и в режиме пользователя.
А вот читатели будут максимально пессимизированы ---
они будут вынуждены ждать пока писатель завершит свою работу, если им не повезло запуститься конкурентно с писателем.
Впрочем, для них это не будет страшно, так как:

- Писатель запускается редко, один раз в секунду для микросхемы RTC. Или двадцать раз в секунду для микросхемы PIT [Intel 8253/8254](https://en.wikipedia.org/wiki/Intel_8253) --- это сконфигурированная в Nikka частота, её можно поменять. Эти два писателя пишут в разные экземпляры структуры, поэтому они никогда не конфликтуют между собой.
- Писатель отрабатывает очень быстро, всего за несколько инструкций процессора.

В качестве инструмента такой неблокирующей синхронизации предлагается использовать упрощённый
[sequence lock](https://en.wikipedia.org/wiki/Seqlock).
В чём-то он будет проще обобщённого
[sequence lock](https://en.wikipedia.org/wiki/Seqlock)
из
[предыдущей задачи](../../lab/book/1-time-4-sequence-lock.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-3--%D1%80%D0%B5%D0%B0%D0%BB%D0%B8%D0%B7%D0%B0%D1%86%D0%B8%D1%8F-sequencelock).
А в чём-то он будет сложнее, --- теперь нам не доступен
[приём read-don't-modify-write](https://www.hpl.hp.com/techreports/2012/HPL-2012-68.pdf).
Так как он требует запись в память, в которую мы не позволим писать из пространства пользователя.

В поле
[`AtomicCorrelationPoint::sequence`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.sequence)
хранится удвоенный номер операции обновления.
А в его младшем бите хранится признак, что сейчас структура
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
находится в неконсистентном состоянии из-за текущей активности писателя.
То есть:

- Нечётное значение в [`AtomicCorrelationPoint::sequence`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.sequence) означает, что писатель начал обновлять структуру [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html), но ещё не закончил. Если читатель обнаруживает структуру в таком состоянии, он должен подождать пока писатель закончит обновление.
- Чётное значение в [`AtomicCorrelationPoint::sequence`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.sequence) означает, что значение структуры [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html) консистентно и читатель может его использовать.

В нашем случае писатель один, что дополнительно упрощает дело.
Алгоритм его действий:
- Раз он один, значит до начала им обновления структуры [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html), её значение заведомо консистентно. А [`AtomicCorrelationPoint::sequence`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.sequence) содержит чётное число.
- Писатель атомарно инкрементирует поле [`AtomicCorrelationPoint::sequence`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.sequence). Оно становится нечётным, символизируя, что идёт обновление.
- После этого писатель заполняет поля [`AtomicCorrelationPoint::count`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.count) и [`AtomicCorrelationPoint::tsc`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.tsc).
- Последним действием писатель атомарно инкрементирует поле [`AtomicCorrelationPoint::sequence`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.sequence). Это действие сигнализирует всем читателям, что обновление завершено и структура находится в консистентном состоянии.

Алгоритм действий читателя чуть сложнее:

- Читатель атомарно загружает значения поля [`AtomicCorrelationPoint::sequence`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.sequence).
- Далее он читает значение полей [`AtomicCorrelationPoint::count`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.count) и [`AtomicCorrelationPoint::tsc`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.tsc).
- После чего он повторно атомарно загружает значение поля [`AtomicCorrelationPoint::sequence`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.sequence).
- Если при обеих загрузках значения поля [`AtomicCorrelationPoint::sequence`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.sequence) совпадают и являются чётными, то структура [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html) была прочитана в консистентном состоянии. Читатель возвращает значение получившейся структуры [`CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html) в вызывающую функцию.
- Если же это не так, читатель повторяет все действия с самого начала.

Перекос сложности в сторону читателя и наличие в нём цикла ожидания, чего нет в писателе, ---
проявление большего приоритета писателя.

Обобщённую реализацию
[sequence lock](https://en.wikipedia.org/wiki/Seqlock)
с несколькими писателями и произвольной защищаемой структурой данных смотрите в
[предыдущей задаче](../../lab/book/1-time-4-sequence-lock.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-3--%D1%80%D0%B5%D0%B0%D0%BB%D0%B8%D0%B7%D0%B0%D1%86%D0%B8%D1%8F-sequencelock).


### Задача 4 --- реализация [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)

Реализуйте описанные алгоритмы чтения и записи.
Читает из структуры
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
её приватный
[метод](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.try_load)

```rust
fn AtomicCorrelationPoint::try_load(&self) -> Option<CorrelationPoint>
```

Он пытается прочитать значение только один раз.
И возвращает его, завернув в
[`core::option::Option::Some`](https://doc.rust-lang.org/nightly/core/option/enum.Option.html#variant.Some),
если оно было прочитано консистентно.

Публичный
[метод](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.load)

```rust
fn AtomicCorrelationPoint::load(&self) -> CorrelationPoint
```

вызывает метод
[`AtomicCorrelationPoint::try_load()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.try_load)
до тех пор пока не удалось прочитать
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
консистентно.
В нём также пригодится функция
[`core::hint::spin_loop()`](https://doc.rust-lang.org/nightly/core/hint/fn.spin_loop.html),
которая сообщает процессору, что он находится в цикле активного ожидания внешнего события.

Пишут в
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
два других метода.
[Метод](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.store)

```rust
fn AtomicCorrelationPoint::store(&self, counter: CorrelationPoint)
```

записывает заданное `counter` значение. А
[метод](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.inc)

```rust
fn AtomicCorrelationPoint::inc(&self, tsc: i64)
```

инкрементирует поле
[`AtomicCorrelationPoint::count`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.count)
и записывает в поле
[`AtomicCorrelationPoint::tsc`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#structfield.tsc)
значение аргумента `tsc`.

Методом
[`AtomicCorrelationPoint::store()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.store)
пользуется обработчик прерываний RTC
[`kernel::time::rtc::interrupt()`](../../doc/kernel/time/rtc/fn.interrupt.html).
С его помощью он сохраняет текущее значение секунд, прошедших с начала Unix--эпохи.
А методом
[`AtomicCorrelationPoint::inc()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.inc) ---
обработчик прерываний PIT
[`kernel::time::pit8254::interrupt()`](../../doc/kernel/time/pit8254/fn.interrupt.html).
Он сохраняет просто счётчик своих тиков, который никак не привязан к реальному времени.
Поэтому ему достаточно инкрементировать логическое значение своего счётчика при каждом прерывании.

Учтите, что в этой задаче точно не нужен
[приём "read-dont-modify-write"](https://www.hpl.hp.com/techreports/2012/HPL-2012-68.pdf).
Как и в предыдущей задаче, его может хотеться использовать в читателе.
Но читатель потенциально будет выполняться в режиме пользователя.
А значит, не сможет выполнять запись в общесистемную структуру
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html),
хранящую показания RTC.
Так как она доступна в режиме пользователя только на чтение.
Другими словами, если вы используете "read-dont-modify-write", то в будущих лабораторках вылезет ошибка.
Не говоря уж о совершенно лишнем захвате кеш--линии в эксклюзивное использование при чтении
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html).

Используйте атомарные поля
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
для синхронизации между собой
[`load()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.load) и
[`store()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.store)/[`inc()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.inc).
То есть, правильно расставьте сами атомарные операции доступа к полям и
[`core::sync::atomic::Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html)
в них.
Чтобы заменить недоступный
[приём read-don't-modify-write](https://www.hpl.hp.com/techreports/2012/HPL-2012-68.pdf)
теперь придётся придумать более хитрую схему синхронизации, чем в
[предыдущей задачи](../../lab/book/1-time-4-sequence-lock.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-3--%D1%80%D0%B5%D0%B0%D0%BB%D0%B8%D0%B7%D0%B0%D1%86%D0%B8%D1%8F-sequencelock).

В отличие от предыдущей задачи, тесты проверяют корректность расстановки
[`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html)
с помощью эмулятора
[Miri](https://github.com/rust-lang/miri).
[Как и раньше](../../lab/book/1-time-3-spinlock.html#%D0%9C%D0%BE%D0%B4%D0%B5%D0%BB%D1%8C-%D0%BF%D0%B0%D0%BC%D1%8F%D1%82%D0%B8-acquire-%D0%B8-release),
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
использовать не рекомендуется.

Вы можете опираться на тот факт, что писатель один и методы записи ---
[`AtomicCorrelationPoint::store()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.store) и
[`AtomicCorrelationPoint::inc()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.inc) ---
гарантированно не вызываются конкурентно ни каждый сам с собой, ни друг с другом.


### Запуск тестов

Тесты можно запустить командой `cargo test --test 1-time-4-correlation-point`
в директориях `ku` и `kernel` репозитория.
В проверочной системе запускаются тесты из обеих этих директорий.


#### Обычный запуск теста [`ku/tests/1-time-4-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-4-correlation-point.rs)

Тест, который находится в файле
[`ku/tests/1-time-4-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-4-correlation-point.rs),
похож на тест из предыдущей задачи.
Это стресс--тест с писателем и читателем, который запускается в многопоточном окружении под linux:

```console
/.../nikka$ (cd ku; cargo test --test 1-time-4-correlation-point)
...
running 3 tests
test correlation_point ... ok
test atomic_correlation_point ... ok
2023-11-05T13:23:34.515701Z : ThreadId(07) consistent_count=124867666 inconsistent_count=0
test single_writer ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.58s
```


#### Запуск теста [`ku/tests/1-time-4-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-4-correlation-point.rs) под [Miri](https://github.com/rust-lang/miri)

Также его можно запустить под эмулятором
[Miri](https://github.com/rust-lang/miri)
промежуточного представления
[Mid-level Intermediate Representation (MIR)](https://rustc-dev-guide.rust-lang.org/mir/index.html)
для Rust'овского кода.


##### Установка [Miri](https://github.com/rust-lang/miri)

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


##### Запуск теста под [Miri](https://github.com/rust-lang/miri)

Теперь можно запустить тест под
[Miri](https://github.com/rust-lang/miri),
дополнительно передав ему параметр `-Zmiri-disable-isolation`:

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


#### Запуск тестов из файла [`kernel/tests/1-time-4-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/1-time-4-correlation-point.rs)

Тесты в файле
[`kernel/tests/1-time-4-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/1-time-4-correlation-point.rs)
запускаются в режиме ядра и используют прерывания для запуска конкурентного кода.

```console
/.../nikka$ (cd kernel; cargo test --test 1-time-4-correlation-point)
...
1_time_4_correlation_point::correlation_point_reader--------
16:38:34 0 D same = 200; different = 107; point = CorrelationPoint { count: 213000, tsc: 426000 }
16:38:35 0 D same = 908; different = 482; point = CorrelationPoint { count: 962000, tsc: 1924000 }
16:38:36 0 D same = 1627; different = 863; point = CorrelationPoint { count: 1725000, tsc: 3450000 }
16:38:37 0 D same = 2346; different = 1244; point = CorrelationPoint { count: 2486000, tsc: 4972000 }
16:38:38 0 D same = 3067; different = 1625; point = CorrelationPoint { count: 3249000, tsc: 6498000 }
16:38:39 0 D same = 3784; different = 2006; point = CorrelationPoint { count: 4010000, tsc: 8020000 }
16:38:40 0 D same = 4503; different = 2387; point = CorrelationPoint { count: 4773000, tsc: 9546000 }
16:38:41 0 D same = 5222; different = 2768; point = CorrelationPoint { count: 5534000, tsc: 11068000 }
16:38:42 0 D same = 5942; different = 3148; point = CorrelationPoint { count: 6294000, tsc: 12588000 }
16:38:43 0 D same = 6651; different = 3525; point = CorrelationPoint { count: 7049000, tsc: 14098000 }
16:38:44 0 D same = 7366; different = 3904; point = CorrelationPoint { count: 7806000, tsc: 15612000 }
16:38:45 0 D same = 8083; different = 4284; point = CorrelationPoint { count: 8565999, tsc: 17131998 }
16:38:46 0 D same = 8800; different = 4663; point = CorrelationPoint { count: 9325000, tsc: 18650000 }
16:38:47 0 D same = 9516; different = 5043; point = CorrelationPoint { count: 10085000, tsc: 20170000 }
16:38:48 0 D same = 10231; different = 5422; point = CorrelationPoint { count: 10842000, tsc: 21684000 }
16:38:49 0 D same = 10948; different = 5802; point = CorrelationPoint { count: 11602000, tsc: 23204000 }
16:38:50 0 D same = 11660; different = 6180; point = CorrelationPoint { count: 12358000, tsc: 24716000 }
16:38:51 0 D same = 12378; different = 6559; point = CorrelationPoint { count: 13117000, tsc: 26234000 }
16:38:52 0 D same = 13091; different = 6938; point = CorrelationPoint { count: 13874000, tsc: 27748000 }
16:38:53 0 D same = 13809; different = 7318; point = CorrelationPoint { count: 14633999, tsc: 29267998 }
16:38:54 0 D same = 14524; different = 7696; point = CorrelationPoint { count: 15390000, tsc: 30780000 }
16:38:55 0 D same = 15228; different = 8070; point = CorrelationPoint { count: 16138000, tsc: 32276000 }
16:38:56 0 D same = 15921; different = 8438; point = CorrelationPoint { count: 16874000, tsc: 33748000 }
16:38:57 0 D same = 16622; different = 8809; point = CorrelationPoint { count: 17617000, tsc: 35234000 }
16:38:58 0 D same = 17299; different = 9168; point = CorrelationPoint { count: 18333999, tsc: 36667998 }
16:38:59 0 D same = 17990; different = 9534; point = CorrelationPoint { count: 19066000, tsc: 38132000 }
16:39:00 0 D same = 18674; different = 9895; point = CorrelationPoint { count: 19789000, tsc: 39578000 }
1_time_4_correlation_point::correlation_point_reader [passed]

1_time_4_correlation_point::correlation_point_writer--------
16:39:00 0 D iteration = 0; failure_count = 0; success_count = 0; point = CorrelationPoint { count: 0, tsc: 0 }
16:39:01 0 D iteration = 966; failure_count = 144083; success_count = 211805; point = CorrelationPoint { count: 966, tsc: 1932 }
16:39:02 0 D iteration = 2325; failure_count = 346723; success_count = 509584; point = CorrelationPoint { count: 2326, tsc: 4652 }
16:39:03 0 D iteration = 3684; failure_count = 549214; success_count = 807221; point = CorrelationPoint { count: 3685, tsc: 7370 }
16:39:04 0 D iteration = 5018; failure_count = 747451; success_count = 1109919; point = CorrelationPoint { count: 5018, tsc: 10036 }
16:39:05 0 D iteration = 5487; failure_count = 807952; success_count = 1473640; point = CorrelationPoint { count: 5487, tsc: 10974 }
16:39:06 0 D iteration = 5959; failure_count = 868840; success_count = 1839664; point = CorrelationPoint { count: 5959, tsc: 11918 }
16:39:07 0 D iteration = 6445; failure_count = 931534; success_count = 2216461; point = CorrelationPoint { count: 6445, tsc: 12890 }
16:39:08 0 D iteration = 6935; failure_count = 994744; success_count = 2595594; point = CorrelationPoint { count: 6935, tsc: 13870 }
16:39:09 0 D iteration = 7424; failure_count = 1057825; success_count = 2974704; point = CorrelationPoint { count: 7424, tsc: 14848 }
16:39:10 0 D iteration = 7914; failure_count = 1121035; success_count = 3354853; point = CorrelationPoint { count: 7914, tsc: 15828 }
16:39:11 0 D iteration = 8404; failure_count = 1184245; success_count = 3734031; point = CorrelationPoint { count: 8404, tsc: 16808 }
16:39:12 0 D iteration = 8874; failure_count = 1244875; success_count = 4098568; point = CorrelationPoint { count: 8874, tsc: 17748 }
16:39:13 0 D iteration = 9332; failure_count = 1303957; success_count = 4453354; point = CorrelationPoint { count: 9332, tsc: 18664 }
16:39:14 0 D iteration = 9792; failure_count = 1363297; success_count = 4810096; point = CorrelationPoint { count: 9792, tsc: 19584 }
16:39:14 0 D iteration = 10000; failure_count = 1390129; success_count = 4970533
1_time_4_correlation_point::correlation_point_writer [passed]
```


> ##### Как устроен тест в [`kernel/tests/1-time-4-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/1-time-4-correlation-point.rs)
>
> В этой задаче дополнительные тесты в файле
> [`kernel/tests/1-time-4-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/1-time-4-correlation-point.rs)
> запускаются в ядре Nikka.
> При этом поддержку нескольких процессоров в нём мы сделаем только в
> [будущей лабе](../../lab/book/3-smp-2-smp-0-intro.html).
> И пока нам не доступна возможность запустить стресс--тест даже на двух процессорах в симметричной системе.
>
> Но можно вспомнить про то, что
> [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
> реализуется конкурентной ради прерываний,
> которые являются одной из конкурентных активностей в компьютере.
> Тогда в голову приходит вариант запустить стресс--тест,
> в котором писатель будет работать в прерывании, а читатель --- в обычном коде, прерываемом периодически этим прерыванием.
> Собственно,
> [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
> предназначен именно для такого сценария использования, в котором прерывание поступает от источника времени --- RTC или PIT.
> Тики RTC, происходящие раз в секунду, точно не подходят для стресс--теста.
> Можно было бы сконфигурировать PIT на его максимальную частоту --- 1.193(18) MHz, но это всё ещё не очень много.
>
> Зато есть интересный режим работы процессора ---
> [режим трассировки](https://en.wikipedia.org/wiki/Stepping_(debugging)).
> Он предназначен для пошаговой отладки программ.
> В этом режиме процессор генерирует прерывание с номером
> [`ku::process::Trap::Debug`](../../doc/ku/process/trap_info/enum.Trap.html#variant.Debug)
> на каждой исполняемой им инструкции программы.
> Задаётся такой режим работы включением
> [флага трассировки](https://en.wikipedia.org/wiki/Trap_flag) в
> [регистре флагов](https://en.wikipedia.org/wiki/FLAGS_register) процессора.
>
> То есть, идея состоит в том, чтобы запустить читателя "под пошаговой отладкой".
> А писателя использовать вместо отладчика.
> Тогда между каждыми двумя инструкциями кода читателя будет запускаться код писателя.
> Правда, так как читатель работает в цикле ожидания корректности структуры
> [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html),
> если писатель будет обновлять её на каждом прерывании --- между каждыми двумя инструкциями читателя, ---
> то читатель никогда не дождётся своего условия выхода из цикла и зависнет.
> Поэтому писатель должен будет на значительное количество итераций такой
> пошаговой отладки прекращать обновление
> [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html).
>
> В коде теста это выглядит так:
> ```rust
> #[test_case]
> fn correlation_point_reader() {
>     interrupts::test_scaffolding::set_debug_handler(writer);
>
>     reader();
>
>     static POINT: AtomicCorrelationPoint = AtomicCorrelationPoint::new();
>
>     ...
> }
> ```
> Функция `interrupts::test_scaffolding::set_debug_handler()` устанавливает заданный обработчик
> прерывания
> [`ku::process::Trap::Debug`](../../doc/ku/process/trap_info/enum.Trap.html#variant.Debug)
> который содержит процедуру писателя `writer()`.
> Дальше запускается читатель `reader()`.
> Конкурировать они будут за переменную `POINT`. Она сделана статической для удобства доступа из
> обработчика прерываний, которому мы не можем передать произвольный набор аргументов.
> По той же причине его внутренне состояние, которое нужно сохранять между вызовами обработчика,
> тоже записывается в статические переменные.
> Переменные, которые использует, в том числе, обработчик прерывания, дополнительно сделаны атомарными.
> Так как формально к ним есть конкурентный доступ.
> Плюс Rust не даст работать с изменяемой статической переменной без `unsafe`,
> чтобы защищать от непредумышленных гонок.
> А вот атомарные типы
> [помечены](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicUsize.html#impl-Sync-for-AtomicUsize)
> типажём
> [`core::marker::Sync`](https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html),
> [что означает](https://doc.rust-lang.org/nomicon/send-and-sync.html)
> корректность их конкурентного использования.
> К сожалению, это приводит к страшно выглядящему коду.
>
> Писатель имеет внутренний счётчик запусков `VALUE`, который и определяет, что делать:
>
> ```rust
> extern "x86-interrupt" fn writer(_: TrapContext) {
>     static VALUE: AtomicUsize = AtomicUsize::new(0);
>
>     let value = VALUE.fetch_add(1, Ordering::Relaxed);
>
>     match (value / 1_000) % 4 {
>         0 => POINT.store(time::test_scaffolding::equal_point(value as i64 + 1)),
>         2 => POINT.inc(POINT.load().count() + 1),
>         _ => {},
>     }
> }
> ```
>
> Сигнатуру `extern "x86-interrupt" fn writer(_: TrapContext)`
> [обсудим чуть позже](../../lab/book/1-time-7-interrupts.html).
>
> Каждую тысячу своих запусков писатель переключается между режимами:
>
> - Записи в `POINT` через вызов [`AtomicCorrelationPoint::store()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.store).
> - Бездействия, чтобы дать шанс читателю увидеть [`AtomicCorrelationPoint::store()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.store) в одном и том же состоянии на протяжении всего цикла своей работы.
> - Записи в `POINT` через вызов [`AtomicCorrelationPoint::inc()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.inc).
> - Очередного бездействия.
>
> При этом писатель и читатель придерживаются соглашения, что консистентными состояниями
> [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
> являются только состояния с одинаковыми значениями полей
> [`CorrelationPoint::tsc`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.tsc) и
> [`CorrelationPoint::count`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.count).
> Поэтому в циклах обновления писатель меняет оба поля на одинаковые значения, отличающиеся от значений при предыдущей записи.
> В частности функция
> `time::test_scaffolding::equal_point()`
> создаёт
> [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
> с одинаковыми
> [`CorrelationPoint::tsc`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.tsc) и
> [`CorrelationPoint::count`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.count),
> равными заданному значению.
> Это имеет смысл только для тестов, поэтому она убрана в модуль `test_scaffolding`.
>
> Читатель устроен так:
>
> ```rust
> fn reader() {
>     ...
>     while ... {
>         switch_trap_flag();
>         let point = POINT.load();
>         switch_trap_flag();
>         ...
>         assert!(point.count() == point.tsc(), "{:?} is inconsistent", point);
>         ...
>     }
> }
> ```
>
> Он включает режим трассировки функцией `switch_trap_flag()`, которая просто переключает состояние
> [флага трассировки](https://en.wikipedia.org/wiki/Trap_flag)
> процессора.
> После этого запускает тестируемый алгоритм чтения
> [`AtomicCorrelationPoint::load()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.load).
> Который должен вернуть управление, только когда писатель прекратит обновлять `POINT` и перейдёт в режим бездействия.
> После чего читатель отключает трассировку повторным переключением флаг `switch_trap_flag()` ---
> когда она включена, он работает очень медленно.
> И проверяет консистентность полученного от
> [`AtomicCorrelationPoint::load()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.load)
> значения
> [`CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html).
>
> Нетрудно догадаться, что в этом тесте мы никогда не прерываем писателя и каждый вызов
> [`AtomicCorrelationPoint::store()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.store) или
> [`AtomicCorrelationPoint::inc()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.inc)
> выполняется атомарно с точки зрения читателя.
> А значит, их корректность мы не проверили.
> Это делает другой тест:
>
> ```rust
> #[test_case]
> fn correlation_point_writer() {
>     interrupts::test_scaffolding::set_debug_handler(reader);
>
>     writer();
>     ...
> }
> ```
>
> Он аналогичен рассмотренному `correlation_point_reader()`, только меняет ролями читателя и писателя.
> Теперь писатель запускается "под пошаговой отладкой", а в обработчике прерывания работает читатель.
>
> Видно, что никак не проверяется случай, когда в едином конкурентном исполнении и читатель прерывается писателем, и наоборот, писатель прерывается читателем.
> Также видно, что такой тест не сможет проверить правильность расстановки `Ordering` в них,
> так как с точки зрения пошаговой отладки все инструкции процессора атомарны с наиболее строгой гарантией консистентности.
>
>> Теперь вы можете исследовать код, запуская его в пошаговом режиме.
>> Например:
>>
>> ```rust
>> #[test_case]
>> fn down_the_rabbit_hole() {
>>     use ku::memory::{Block, Virt};
>>
>>     interrupts::test_scaffolding::set_debug_handler(collect_statistics);
>>
>>     switch_trap_flag();
>>     debug!("how many instructions does it take to log something?");
>>     switch_trap_flag();
>>
>>     let max_rsp = MAX_RSP.load(Ordering::Relaxed);
>>     let min_rsp = cmp::min(max_rsp, MIN_RSP.load(Ordering::Relaxed));
>>     let rip = RIP.load(Ordering::Relaxed);
>>
>>     debug!(
>>         instruction_count = INSTRUCTION_COUNT.load(Ordering::Relaxed),
>>         used_stack_space = ?Block::<Virt>::from_index(min_rsp, max_rsp),
>>         last_traced_insruction_address = ?Virt::new(rip),
>>     );
>>
>>     static INSTRUCTION_COUNT: AtomicUsize = AtomicUsize::new(0);
>>     static MAX_RSP: AtomicUsize = AtomicUsize::new(usize::MIN);
>>     static MIN_RSP: AtomicUsize = AtomicUsize::new(usize::MAX);
>>     static RIP: AtomicUsize = AtomicUsize::new(0);
>>
>>     extern "x86-interrupt" fn collect_statistics(context: TrapContext) {
>>         let context = context.get().mini_context();
>>         let rip = context.rip().into_usize();
>>         let rsp = context.rsp().into_usize();
>>
>>         let max_rsp = cmp::max(rsp, MAX_RSP.load(Ordering::Relaxed));
>>         let min_rsp = cmp::min(rsp, MIN_RSP.load(Ordering::Relaxed));
>>
>>         INSTRUCTION_COUNT.fetch_add(1, Ordering::Relaxed);
>>         MAX_RSP.store(max_rsp, Ordering::Relaxed);
>>         MIN_RSP.store(min_rsp, Ordering::Relaxed);
>>         RIP.store(rip, Ordering::Relaxed);
>>     }
>> }
>> ```
>>
>> В отладочной сборке на логирование одной строки ушло 144218 инструкций процессора,
>> последняя из них располагалась по адресу `0v28_838E`.
>> А также было потрачено 4.203 KiB стека по адресам `[0v100_0200_0000, 0v100_0200_10D0)`
>> ```console
>> /.../nikka$ (cd kernel; cargo test --test 1-time-4-correlation-point)
>> ...
>> 1_time_4_correlation_point::down_the_rabbit_hole------------
>> 16:41:38 0 D waiting for the RTC to tick twice
>> 16:41:40 0 D how many instructions does it take to log something?; rflags = IF PF
>> 16:41:40 0 D how many instructions does it take to log something?; rflags = IF TF PF
>> 16:41:40 0 D instruction_count = 144218; used_stack_space = [0v100_0200_0000, 0v100_0200_10D0), size 4.203 KiB; last_traced_insruction_address = 0v28_838E
>> 16:41:40 0 D time_to_log_a_message = 5.422 ms; time_to_log_a_message_in_the_stepping_mode = 621.737 ms; stepping_slowdown_ratio = 114.67242158650633
>> 1_time_4_correlation_point::down_the_rabbit_hole--- [passed]
>> 16:41:40 0 I exit qemu; exit_code = ExitCode(SUCCESS)
>> ```
>> Возможно вы заметили, что второе сообщение `how many ...`, которое печаталось под трассировкой, появлялось на экране медленнее.
>>
>> В релизной сборке на логирование одной строки ушло 10503 инструкций и 1.000 KiB стека:
>>
>> ```console
>> /.../nikka$ (cd kernel; cargo test --test 1-time-4-correlation-point --release)
>> ...
>> 1_time_4_correlation_point::down_the_rabbit_hole------------
>> 16:45:53 0 D waiting for the RTC to tick twice
>> 16:45:55 0 D how many instructions does it take to log something?; rflags = IF
>> 16:45:55 0 D how many instructions does it take to log something?; rflags = IF TF
>> 16:45:55 0 D instruction_count = 10503; used_stack_space = [0v100_0200_1990, 0v100_0200_1D90), size 1.000 KiB; last_traced_insruction_address = 0v21_BD59
>> 16:45:55 0 D time_to_log_a_message = 1.654 ms; time_to_log_a_message_in_the_stepping_mode = 37.214 ms; stepping_slowdown_ratio = 22.494754670444543
>> 1_time_4_correlation_point::down_the_rabbit_hole--- [passed]
>> ```
>>
>>> Теперь вы можете, например, построить гистограмму количества раз, сколько исполнялись инструкции по разным адресам.
>>> И найти самые горячие циклы кода.


### Theory and Practice of Concurrency

Более подробно познакомиться с конкурентностью, атомарными переменными, синхронизациями, моделями памяти и т.п.,
вы можете на курсе Ромы Липовского "Theory and Practice of Concurrency":

- Публичный доступ:
  - [Плейлист с лекциями](https://youtube.com/playlist?list=PL4_hYwCyhAva37lNnoMuBcKRELso5nvBm).
  - [Репозиторий курса](https://gitlab.com/Lipovsky/concurrency-course).
- Личный кабинет ШАД, [весна 2023](https://lk.yandexdataschool.ru/courses/2023-spring/7.1141-theory-and-practice-of-concurrency/classes/) или [весна 2022](https://lk.yandexdataschool.ru/courses/2022-spring/7.1013-theory-and-practice-of-concurrency/classes/).
- [Wiki yandex-team](https://wiki.yandex-team.ru/hr/volnoslushateli/lekcii-shad/).


### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/time/correlation_point.rs | 55 +++++++++++++++++++++++++++++++++++++++++++++----------
 1 file changed, 45 insertions(+), 10 deletions(-)
```
