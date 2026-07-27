## Измерение частоты процессора и повышение разрешения часов

Структура
[`ku::time::correlation_interval::CorrelationInterval`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html)
из файла [`ku/src/time/correlation_interval.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/time/correlation_interval.rs)
предназначена для соотнесения частоты процессора и другого источника времени.
Она содержит поля:

- [`CorrelationInterval::base`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#structfield.base) со знакомым нам [`CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html) в некоторый базовый момент времени, когда тикнули отслеживаемые [`CorrelationInterval`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html) часы.
- [`CorrelationInterval::prev`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#structfield.prev) с [`CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html) на момент последнего тика отслеживаемых часов.

[`CorrelationInterval::base`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#structfield.base) и
[`CorrelationInterval::prev`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#structfield.prev)
содержат количество тиков отслеживаемых часов и номер такта процессора в соответствующие моменты времени.
Частота отслеживаемых часов задаётся как константный параметр `TICKS_PER_SECOND` для типа
[`struct CorrelationInterval<const TICKS_PER_SECOND: i64>`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html).

По этой информации можно:

- Вычислить частоту процессора с точки зрения отслеживаемых часов.
- Пересчитать произвольное значение счётчика тактов процессора [`ku::time::tsc::Tsc`](../../doc/ku/time/tsc/struct.Tsc.html) в показания времени отслеживаемых часов. При этом мы фактически повышаем разрешение отслеживаемых часов до частоты процессора. Разумеется, погрешность при этом превышает разрешение.

Для часов реального времени
[`ku::time::rtc::Rtc`](../../doc/ku/time/rtc/struct.Rtc.html),
количество тиков в каждом поле
[`CorrelationPoint::count`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html#structfield.count)
содержит количество секунд с начала Unix--эпохи.
Методы
[`CorrelationInterval`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html)
будут опираться на это при привязке своих
[`CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html)
к реальному времени.

Структура
[`ku::time::correlation_interval::AtomicCorrelationInterval`](../../doc/ku/time/correlation_interval/struct.AtomicCorrelationInterval.html)
содержит аналогичные два поля типа
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html).
Она находится в таких же отношениях с
[`CorrelationInterval`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html),
как
[`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
с
[`CorrelationPoint`](../../doc/ku/time/correlation_point/struct.CorrelationPoint.html).
То есть,
[`CorrelationInterval`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html)
является значением, а
[`AtomicCorrelationInterval`](../../doc/ku/time/correlation_interval/struct.AtomicCorrelationInterval.html)
его конкурентным хранилищем в памяти.

> Подумайте, почему при этом
> [`AtomicCorrelationInterval`](../../doc/ku/time/correlation_interval/struct.AtomicCorrelationInterval.html)
> устроен гораздо проще чем
> [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)
> с его [sequence lock](https://en.wikipedia.org/wiki/Seqlock).

Основной [метод](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#method.datetime)

```rust
fn CorrelationInterval::datetime<const PARTS_PER_SECOND: i64>(
    atomic_correlation_interval: &AtomicCorrelationInterval<TICKS_PER_SECOND>,
    tsc: Tsc,
) -> DateTime<Utc>
```

должен для часов, к которым привязан `atomic_correlation_interval`, выдать время, соответствующее
такту процессора, записанному в `tsc`.
Время он возвращает в типе
[`chrono::DateTime`](../../doc/chrono/struct.DateTime.html).
Он считает, что заданные ему часы показывают количество секунд, прошедших с начала Unix--эпохи в UTC ---
[`chrono::offset::Utc`](../../doc/chrono/offset/struct.Utc.html).

Функции
[`ku::time::datetime()`](../../doc/ku/time/fn.datetime.html) и
[`ku::time::datetime_ms()`](../../doc/ku/time/fn.datetime_ms.html)
повышают разрешение часов реального времени
[`ku::time::rtc::Rtc`](../../doc/ku/time/rtc/struct.Rtc.html),
используя этот метод:

```rust
pub fn datetime(tsc: Tsc) -> DateTime<Utc> {
    Rtc::datetime::<NSECS_PER_SEC>(tsc)
}

pub fn datetime_ms(tsc: Tsc) -> DateTime<Utc> {
    Rtc::datetime::<MSECS_PER_SEC>(tsc)
}

const MSECS_PER_SEC: i64 = 1_000;
const NSECS_PER_SEC: i64 = 1_000_000_000;
```

Метод
[`CorrelationInterval::datetime()`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#method.datetime)
читает переданный ему на вход `atomic_correlation_interval`.
И проверяет, что в результирующем
[`CorrelationInterval`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html)
оба поля ---
[`CorrelationInterval::base`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#structfield.base) и
[`CorrelationInterval::prev`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#structfield.prev) ---
валидны и задают разные тики базовых часов.
Если это так, он перекладывает основную работу на вспомогательный
[метод](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#method.datetime_with_resolution)

```rust
fn CorrelationInterval::datetime_with_resolution<const PARTS_PER_SECOND: i64>(
    &self,
    tsc: Tsc,
) -> NaiveDateTime
```

В тех редких случаях, когда в `atomic_correlation_interval` ещё не прошло два тика часов, метод
[`CorrelationInterval::datetime()`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#method.datetime)
возвращает момент времени
[`CorrelationInterval::prev`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#structfield.prev)
в виде
[`DateTime`](../../doc/chrono/struct.DateTime.html).
Мы заметим это на старте системы по низкому --- секундному --- разрешению в записях лога.


### Задача 5 --- реализация основного метода повышения разрешения часов

Для этой задачи понадобится готовое решение [задачи 1](../../lab/book/1-time-1-rtc.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-1--%D1%87%D1%82%D0%B5%D0%BD%D0%B8%D0%B5-%D1%80%D0%B5%D0%B0%D0%BB%D1%8C%D0%BD%D0%BE%D0%B3%D0%BE-%D0%B2%D1%80%D0%B5%D0%BC%D0%B5%D0%BD%D0%B8-%D0%B8%D0%B7-%D0%BC%D0%B8%D0%BA%D1%80%D0%BE%D1%81%D1%85%D0%B5%D0%BC%D1%8B-rtc).

Реализуйте метод
[`fn CorrelationInterval::datetime_with_resolution()`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#method.datetime_with_resolution).

- Считайте, что `self.base.count()` и `self.prev.count()` задают разные секунды с начала Unix--эпохи.
- Необходимо вернуть реальное время, которое соответствует такту номер `tsc.get()` процессора. Он может быть как больше `self.prev.tsc()`, так и меньше `self.base.tsc()`, а может лежать где-то между ними. Чтобы все эти варианты можно было поддержать, такты процессора хранятся в знаковом типе [`i64`](https://doc.rust-lang.org/nightly/core/primitive.i64.html).
- Результат возвращается в типе [`chrono::naive::NaiveDateTime`](../../doc/chrono/naive/struct.NaiveDateTime.html), который не содержит данных о часовом поясе. Идентификатор часового пояса добавит вызывающая функция.
- Запрошенное разрешение `PARTS_PER_SECOND` задаётся в единицах в секунду, например для миллисекунд `PARTS_PER_SECOND = 1_000`. Оно не будет выше наносекунд --- `PARTS_PER_SECOND = 1_000_000_000`, --- которые поддерживает [`NaiveDateTime`](../../doc/chrono/naive/struct.NaiveDateTime.html).
- [Високосные секунды](https://en.wikipedia.org/wiki/Leap_second) не поддерживаем.
- Вам могут пригодиться функции [`i64::div_euclid()`](https://doc.rust-lang.org/nightly/std/primitive.i64.html#method.div_euclid) и [`i64::rem_euclid()`](https://doc.rust-lang.org/nightly/std/primitive.i64.html#method.rem_euclid).


#### Зависание или паника при логировании

Если попробовать добавить логирование в метод
[`CorrelationInterval::datetime_with_resolution()`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#method.datetime_with_resolution),
то код зависнет.
Логирование само использует
[`CorrelationInterval::datetime_with_resolution()`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#method.datetime_with_resolution),
поэтому можно было бы ожидать, что возникнет бесконечная рекурсия:
- `debug!()` или другой макрос логирования вызывает `datetime_with_resolution()`.
- Который вызывает `debug!()` или другой макрос логирования.
- Который вызывает `datetime_with_resolution()`.
- ...
- В конце концов произошло бы исчерпание стека и падение ядра по Page Fault.

Бесконечной рекурсии не происходит из-за дополнительной блокировки в логировании --- поле
[`kernel::log::LogCollector::log`](../../doc/kernel/log/struct.LogCollector.html#structfield.log)
имеет тип
[`ku::sync::Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)`<`[`kernel::log::Log`](../../doc/kernel/log/struct.Log.html)`>`.
Основное предназначение этой блокировки ---
предотвратить перемешивание кусков сообщений от разных процессоров в системе.

А вот бесконечную рекурсию она переводит во [взаимоблокировку](https://en.wikipedia.org/wiki/Deadlock).
Когда происходит вызов логирования, блокировка захватывается.
С захваченной блокировкой происходит вызов
[`CorrelationInterval::datetime_with_resolution()`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#method.datetime_with_resolution).
Если он сам вызывает логирование, то происходит попытка рекурсивного захвата уже захваченной блокировки.
Она не поддерживает рекурсивный захват, поэтому происходит
[взаимоблокировка](https://en.wikipedia.org/wiki/Deadlock),
которая и проявляется как зависание.

Если вы сделали задачи на Spinlock и SequenceLock, то это зависание приведёт к панике с отладочным сообщением:
```console
...
     Running tests/1-time-5-high-resolution-date.rs (/.../nikka/target/kernel/debug/deps/1_time_5_high_resolution_date-683176186a322834)
...
1_time_5_high_resolution_date::high_resolution_date---------
17:14:41 0 D waiting for the RTC to tick twice
panicked at kernel/src/log.rs:322:18:
deadlock detected on spinlock defined at kernel/src/log.rs:308:18, locked at kernel/src/log.rs:322:18, lock attempt at kernel/src/log.rs:322:18, backtrace: [0x22D7C8 0x29843E 0x2984A3 0x298353 0x298470 0x290FCA 0x290697 0x28FA61 0x293279 0x292928 0x22CF42]
--------------------------------------------------- [failed]
```
С помощью `llvm-symbolizer` можно расшифровать полученный backtrace:
```console
/.../nikka$ echo '0x22D7C8 0x29843E 0x2984A3 0x298353 0x298470 0x290FCA 0x290697 0x28FA61 0x293279 0x292928 0x22CF42' | tr -d ',_' | tr 'v ' 'x\n' | llvm-symbolizer --exe /.../nikka/target/kernel/debug/deps/1_time_5_high_resolution_date-683176186a322834
_$LT$kernel..log..LogCollector$u20$as$u20$tracing_core..collect..Collect$GT$::event::h462d99e420ca2493
/.../nikka/kernel/src/log.rs:322:9

tracing_core::dispatch::Dispatch::event::h67eb2c170b121b93
/.../.cargo/git/checkouts/tracing-e9bbb56ea31f0c18/b8c45cc/tracing-core/src/dispatch.rs:752:13

tracing_core::event::Event::dispatch::_$u7b$$u7b$closure$u7d$$u7d$::h01d7a0f38d1ad551
/.../.cargo/git/checkouts/tracing-e9bbb56ea31f0c18/b8c45cc/tracing-core/src/event.rs:36:10

tracing_core::dispatch::get_default::hff5d99b033e23eef
/.../.cargo/git/checkouts/tracing-e9bbb56ea31f0c18/b8c45cc/tracing-core/src/dispatch.rs:501:2

tracing_core::event::Event::dispatch::hd7df4b7bcd805a1a
/.../.cargo/git/checkouts/tracing-e9bbb56ea31f0c18/b8c45cc/tracing-core/src/event.rs:37:6

ku::time::correlation_interval::CorrelationInterval$LT$_$GT$::datetime_with_resolution::_$u7b$$u7b$closure$u7d$$u7d$::h41d9a946112ee058
/.../.cargo/git/checkouts/tracing-e9bbb56ea31f0c18/b8c45cc/tracing/src/macros.rs:885:15

ku::time::correlation_interval::CorrelationInterval$LT$_$GT$::datetime_with_resolution::hbfbe0a0ed7dac3e1
/.../nikka/ku/src/time/correlation_interval.rs:62:9

ku::time::correlation_interval::CorrelationInterval$LT$_$GT$::datetime::h8662a6334f24af94
/.../nikka/ku/src/time/correlation_interval.rs:46:13

ku::time::rtc::Rtc::datetime::hb639862c4f2d4b5c
/.../nikka/ku/src/time/rtc.rs:22:9

ku::time::datetime_ms::h58c326396bd47ace
/.../nikka/ku/src/time/mod.rs:45:5

kernel::log::Log::log_metadata::hf3b3dd5726499f9b
/.../nikka/kernel/src/log.rs:211:13
```
В нём видно проблемную строку:
```console
ku::time::correlation_interval::CorrelationInterval$LT$_$GT$::datetime_with_resolution::hbfbe0a0ed7dac3e1
/.../nikka/ku/src/time/correlation_interval.rs:62:9
```
```console
/.../nikka$ nl -b a /.../nikka/ku/src/time/correlation_interval.rs | grep -w 62
    62          debug!("create a deadlock");
```

##### Или можно запустить тест под gdb

Если вы не реализовали отладку взаимоблокировок,
то факт [взаимоблокировки](https://en.wikipedia.org/wiki/Deadlock),
какую конкретно блокировку и какая строка кода пытается захватить,
можно узнать с помощью `gdb`.
Из ранее запущенной команды теста
```console
/.../nikka$ (cd kernel; cargo test --test 1-time-5-high-resolution-date)
...
     Running tests/1-time-5-high-resolution-date.rs (/.../nikka/target/kernel/debug/deps/1_time_5_high_resolution_date-683176186a322834)
...
Running: `qemu-system-x86_64 -drive format=raw,file=/.../nikka/target/kernel/debug/deps/bootimage-1_time_5_high_resolution_date-683176186a322834.bin -no-reboot -m size=128M -smp cpus=4 -device isa-debug-exit,iobase=0xF4,iosize=0x04 -serial stdio -display none -drive file=../fs.img,index=1,media=disk,driver=raw`
```
нам понадобятся две строки --- `Running tests/...` с ELF--файлом теста и
`Running: 'qemu-system-x86_64...` с командой запуска эмулятора.

В одной консоли запустим qemu, добавив в его команду аргументы `-gdb tcp::1234 -S`.
Они означают, что нужно дождаться отладчика gdb на заданном порту:

```console
/.../nikka$ qemu-system-x86_64 -drive format=raw,file=/.../nikka/target/kernel/debug/deps/bootimage-1_time_5_high_resolution_date-683176186a322834.bin -no-reboot -m size=128M -smp cpus=4 -device isa-debug-exit,iobase=0xF4,iosize=0x04 -serial stdio -display none -drive file=../fs.img,index=1,media=disk,driver=raw -gdb tcp::1234 -S
...
```

Или через `cargo test`:

```console
/.../nikka$ cargo test --test 1-time-5-high-resolution-date -- -gdb tcp::1234 -S
...
```

В другой консоли создадим командный файл для gdb с указанием ELF--файла теста:
```console
/.../nikka$ cat > gdbinit-test <<EOF
set architecture i386:x86-64
set disassembly-flavor intel
set print asm-demangle on
target remote localhost:1234
file /.../nikka/target/kernel/debug/deps/1_time_5_high_resolution_date-683176186a322834
EOF
```

Запустим gdb с этим командным файлом, дадим команду `continue`, чтобы эмулятор запустил ядро.
А когда оно зависнет, нажмём `<Ctrl-C>` и выполним команду `backtrace`.
Использование `peda` может привести к тому, что `<Ctrl-C>` не отработает нормально.
Если возникает такая проблема, закомментируйте строчку `source ~/peda/peda.py` в `~/.gdbinit`.
```console
~/nikka$ gdb -command=gdbinit-test
...
0x000000000000fff0 in ?? ()
(gdb) continue
Continuing.
^C
Thread 1 received signal SIGINT, Interrupt.
core::core_arch::x86::sse2::_mm_pause () at /.../.rustup/toolchains/nightly-2023-09-16-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/../../stdarch/crates/core_arch/src/x86/sse2.rs:26
26 }
(gdb) backtrace
#0  core::core_arch::x86::sse2::_mm_pause ()
    at /.../.rustup/toolchains/nightly-2023-09-16-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/../../stdarch/crates/core_arch/src/x86/sse2.rs:26
#1  0x0000000000270b59 in core::hint::spin_loop ()
    at /.../.rustup/toolchains/nightly-2023-09-16-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/hint.rs:175
#2  core::sync::atomic::spin_loop_hint ()
    at /.../.rustup/toolchains/nightly-2023-09-16-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/sync/atomic.rs:3652
#3  0x0000000000235995 in spin::relax::{impl#0}::relax ()
    at /.../.cargo/registry/src/index.crates.io-6f17d22bba15001f/spin-0.9.8/src/relax.rs:29
#4  spin::mutex::spin::SpinMutex<kernel::log::Log, spin::relax::Spin>::lock<kernel::log::Log, spin::relax::Spin> (
    self=0x2e1938 <kernel::log::LOG_COLLECTOR+184>)
    at /.../.cargo/registry/src/index.crates.io-6f17d22bba15001f/spin-0.9.8/src/mutex/spin.rs:187
#5  spin::mutex::Mutex<kernel::log::Log, spin::relax::Spin>::lock<kernel::log::Log, spin::relax::Spin> (
    self=0x2e1938 <kernel::log::LOG_COLLECTOR+184>)
    at /.../.cargo/registry/src/index.crates.io-6f17d22bba15001f/spin-0.9.8/src/mutex.rs:186
#6  ku::sync::spinlock::Spinlock<kernel::log::Log>::lock<kernel::log::Log> (
    self=0x2e1880 <kernel::log::LOG_COLLECTOR>) at ku/src/sync/spinlock.rs:163
#7  0x000000000022d338 in kernel::log::{impl#4}::event (self=0x2e1880 <kernel::log::LOG_COLLECTOR>, 
    event=0x100001ffb48) at kernel/src/log.rs:322
#8  0x000000000029418e in tracing_core::dispatch::Dispatch::event (
    self=0x2e2840 <tracing_core::dispatch::GLOBAL_DISPATCH>, event=0x100001ffb48) at src/dispatch.rs:752
#9  0x00000000002941f3 in tracing_core::event::{impl#0}::dispatch::{closure#0} (
    current=0x2e2840 <tracing_core::dispatch::GLOBAL_DISPATCH>) at src/event.rs:35
#10 0x00000000002940a3 in tracing_core::dispatch::get_default<(), tracing_core::event::{impl#0}::dispatch::{closure_env#0}> (f=...) at src/dispatch.rs:500
#11 0x00000000002941c0 in tracing_core::event::Event::dispatch (
--Type <RET> for more, q to quit, c to continue without paging--
    metadata=0x2d9468 <ku::time::correlation_interval::CorrelationInterval<_>::datetime_with_resolution::__CALLSITE::__META>, fields=0x100001ffc48) at src/event.rs:34
#12 0x000000000028df2a in ku::time::correlation_interval::{impl#0}::datetime_with_resolution::{closure#0}<1, 1000> (
    value_set=<error reading variable: Cannot access memory at address 0x0>)
    at /.../.cargo/git/checkouts/tracing-e9bbb56ea31f0c18/b8c45cc/tracing/src/macros.rs:876
#13 0x000000000028d5f7 in ku::time::correlation_interval::CorrelationInterval<1>::datetime_with_resolution<1, 1000> (
    self=0x100001ffed8, tsc=...) at ku/src/time/correlation_interval.rs:62
#14 0x000000000028c9c1 in ku::time::correlation_interval::CorrelationInterval<1>::datetime<1, 1000> (
    atomic_correlation_interval=0x324030 <kernel::SYSTEM_INFO+48>, tsc=...) at ku/src/time/correlation_interval.rs:46
#15 0x000000000028f1d9 in ku::time::rtc::Rtc::datetime<1000> (tsc=...) at ku/src/time/rtc.rs:22
#16 0x000000000028e888 in ku::time::datetime_ms (tsc=...) at ku/src/time/mod.rs:45
#17 0x000000000022cab2 in kernel::log::Log::log_metadata (self=0x2e1939 <kernel::log::LOG_COLLECTOR+185>, 
    level=0x2d9468 <ku::time::correlation_interval::CorrelationInterval<_>::datetime_with_resolution::__CALLSITE::__META>, metadata=...) at kernel/src/log.rs:210
#18 0x000000000022ca19 in kernel::log::Log::log_event (self=0x2e1939 <kernel::log::LOG_COLLECTOR+185>, 
    event=0x100002001b8, timestamp=...) at kernel/src/log.rs:197
#19 0x000000000022d359 in kernel::log::{impl#4}::event (self=0x2e1880 <kernel::log::LOG_COLLECTOR>, 
    event=0x100002001b8) at kernel/src/log.rs:322
#20 0x000000000029418e in tracing_core::dispatch::Dispatch::event (
    self=0x2e2840 <tracing_core::dispatch::GLOBAL_DISPATCH>, event=0x100002001b8) at src/dispatch.rs:752
#21 0x00000000002941f3 in tracing_core::event::{impl#0}::dispatch::{closure#0} (
    current=0x2e2840 <tracing_core::dispatch::GLOBAL_DISPATCH>) at src/event.rs:35
#22 0x00000000002940a3 in tracing_core::dispatch::get_default<(), tracing_core::event::{impl#0}::dispatch::{closure_env#0}> (f=...) at src/dispatch.rs:500
#23 0x00000000002941c0 in tracing_core::event::Event::dispatch (
    metadata=0x2d9468 <ku::time::correlation_interval::CorrelationInterval<_>::datetime_with_resolution::__CALLSITE::__META>, fields=0x100002002b8) at src/event.rs:34
#24 0x000000000028defa in ku::time::correlation_interval::{impl#0}::datetime_with_resolution::{closure#0}<1, 1000000000--Type <RET> for more, q to quit, c to continue without paging--
> (value_set=<error reading variable: Cannot access memory at address 0x0>)
    at /.../.cargo/git/checkouts/tracing-e9bbb56ea31f0c18/b8c45cc/tracing/src/macros.rs:876
#25 0x000000000028cf87 in ku::time::correlation_interval::CorrelationInterval<1>::datetime_with_resolution<1, 1000000000> (self=0x10000200548, tsc=...) at ku/src/time/correlation_interval.rs:62
#26 0x000000000028cac1 in ku::time::correlation_interval::CorrelationInterval<1>::datetime<1, 1000000000> (
    atomic_correlation_interval=0x324030 <kernel::SYSTEM_INFO+48>, tsc=...) at ku/src/time/correlation_interval.rs:46
#27 0x000000000028f189 in ku::time::rtc::Rtc::datetime<1000000000> (tsc=...) at ku/src/time/rtc.rs:22
#28 0x000000000028e858 in ku::time::datetime (tsc=...) at ku/src/time/mod.rs:39
#29 0x00000000002570a7 in kernel::time::rtc::interrupt () at kernel/src/time/rtc.rs:31
#30 0x000000000026f518 in kernel::trap::rtc (_context=...) at kernel/src/trap.rs:824
#31 0x0000000000216b85 in x86_64::instructions::hlt ()
    at /.../.cargo/registry/src/index.crates.io-6f17d22bba15001f/x86_64-0.14.10/src/instructions/mod.rs:18
#32 0x00000000002174f9 in 1_time_5_high_resolution_date::wait_for_two_correlation_points ()
    at kernel/tests/1-time-5-high-resolution-date.rs:195
#33 0x0000000000218e10 in 1_time_5_high_resolution_date::high_resolution_date ()
    at kernel/tests/1-time-5-high-resolution-date.rs:121
#34 0x0000000000216f41 in core::ops::function::Fn::call<fn(), ()> ()
    at /.../.rustup/toolchains/nightly-2023-09-16-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/ops/function.rs:79
#35 0x000000000021b6ce in kernel::{impl#0}::run<fn()> (self=0x200cf8) at kernel/src/lib.rs:247
#36 0x000000000022c156 in kernel::test_runner (tests=...) at kernel/src/lib.rs:258
#37 0x000000000021b225 in 1_time_5_high_resolution_date::test_main ()
    at kernel/tests/1-time-5-high-resolution-date.rs:1
#38 0x000000000021752a in 1_time_5_high_resolution_date::test_entry (boot_info=0x10000000000)
    at kernel/tests/gen_main/mod.rs:12
#39 0x000000000021b20c in 1_time_5_high_resolution_date::__impl_start (boot_info=0x10000000000)
    at /.../.cargo/registry/src/index.crates.io-6f17d22bba15001f/bootloader-0.9.23/src/lib.rs:39
(gdb)
```

Фрейм `#4` говорит нам, что код крутится в `SpinMutex<kernel::log::Log>`, принадлежащей
[`kernel::log::LOG_COLLECTOR`](../../doc/kernel/log/static.LOG_COLLECTOR.html):
```console
#4  spin::mutex::spin::SpinMutex<kernel::log::Log, spin::relax::Spin>::lock<kernel::log::Log, spin::relax::Spin> (
    self=0x2e1938 <kernel::log::LOG_COLLECTOR+184>)
    at /.../.cargo/registry/src/index.crates.io-6f17d22bba15001f/spin-0.9.8/src/mutex/spin.rs:187
```
Фрейм `#5` сообщает, где вызывается повторный захват блокировки --- в строке `kernel/src/log.rs:322` метода `event()`:
```console
#7  0x000000000022d338 in kernel::log::{impl#4}::event (self=0x2e1880 <kernel::log::LOG_COLLECTOR>, 
    event=0x100001ffb48) at kernel/src/log.rs:322
```
А по фрейму `#19` видно, что блокировка уже была захвачена той же строкой кода:
```console
#19 0x000000000022d359 in kernel::log::{impl#4}::event (self=0x2e1880 <kernel::log::LOG_COLLECTOR>, 
    event=0x100002001b8) at kernel/src/log.rs:322
```
А во фреймах `#13` и `#25` видно проблемную строку, которая и привела к зацикливанию и взаимоблокировке:
```console
#13 0x000000000028d5f7 in ku::time::correlation_interval::CorrelationInterval<1>::datetime_with_resolution<1, 1000> (
    self=0x100001ffed8, tsc=...) at ku/src/time/correlation_interval.rs:62
#25 0x000000000028cf87 in ku::time::correlation_interval::CorrelationInterval<1>::datetime_with_resolution<1, 1000000000> (self=0x10000200548, tsc=...) at ku/src/time/correlation_interval.rs:62
```

Отладить метод
[`fn CorrelationInterval::datetime_with_resolution()`](../../doc/ku/time/correlation_interval/struct.CorrelationInterval.html#method.datetime_with_resolution)
с помощью логирования можно, если разорвать бесконечную рекурсию.
Например, не вызывая функции времени из логирования.
Для этого служит формат
[`kernel::log::Format::Timeless`](../../doc/kernel/log/enum.Format.html#variant.Timeless).
Чтобы воспользоваться им, измените строку инициализации
[`kernel::log::LOG_COLLECTOR`](../../doc/kernel/log/static.LOG_COLLECTOR.html)
в конце файла
[`kernel/src/log.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/log.rs):

```rust
static LOG_COLLECTOR: LogCollector = LogCollector::new(Format::Timeless, Level::DEBUG);
```

Также иногда можно убирать
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html):
из поля
[`kernel::log::LogCollector::log`](../../doc/kernel/log/struct.LogCollector.html#structfield.log).
Если на нём возникает [взаимоблокировка](https://en.wikipedia.org/wiki/Deadlock),
а перемешивание записей логов не мешает отладке или вообще не возникает при использовании только одного процессора.


### Запуск тестов

Тесты можно запустить командой `cargo test --test 1-time-5-high-resolution-date` в директории `kernel` репозитория.

После выполнения задачи должен проходить тест `high_resolution_date()`
в файле
[`kernel/tests/time.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/time.rs):

```console
/.../nikka$ (cd kernel; cargo test --test 1-time-5-high-resolution-date)
...
1_time_5_high_resolution_date::high_resolution_date---------
16:50:10 0 D waiting for the RTC to tick twice
16:50:12.003 0 D measuring; timer_ticks = 102
16:50:17.019 0 D measured; samples = 100; min = PT0.043444808S; max = PT0.050378112S; mean = PT0.049998107S; deviation = PT0.000809628S
16:50:17.023 0 D restrictions; min_delta = PT0.040S; max_delta = PT0.060S; min_mean = PT0.049500S; max_mean = PT0.050500S; min_deviation = PT0.000000100S; max_deviation = PT0.005S
1_time_5_high_resolution_date::high_resolution_date [passed]

1_time_5_high_resolution_date::points_beyond_interval-------
16:50:17.027 0 D waiting for the RTC to tick twice
16:50:19.001 0 D before_boot = 2023-09-17 16:50:07.091008303 UTC
16:50:19.007 0 D boot = 2023-09-17 16:50:10.008804597 UTC
16:50:19.011 0 D now = 2023-09-17 16:50:19.000589843 UTC
16:50:19.013 0 D future = 2023-09-17 16:50:21.918491449 UTC
16:50:19.015 0 D left = 2023-09-17 16:50:07.091008303 UTC; right = 2023-09-17 16:50:10.008804597 UTC; difference = PT2.917796294S
16:50:19.029 0 D left = 2023-09-17 16:50:10.008804597 UTC; right = 2023-09-17 16:50:19.000589843 UTC; difference = PT8.991785246S
16:50:19.041 0 D left = 2023-09-17 16:50:19.000589843 UTC; right = 2023-09-17 16:50:21.918491449 UTC; difference = PT2.917901606S
1_time_5_high_resolution_date::points_beyond_interval [passed]

1_time_5_high_resolution_date::stress-----------------------
16:50:19.065 0 D waiting for the RTC to tick twice
1_time_5_high_resolution_date::stress-------------- [passed]
```

### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/time/correlation_interval.rs | 26 ++++++++++++++++++++++++--
 1 file changed, 24 insertions(+), 2 deletions(-)
```
