## Таблица процессов

Код таблицы процессов находится в файле [`kernel/src/process/table.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/table.rs).


### Идентификаторы процессов [`ku::process::pid::Pid`](../../doc/ku/process/pid/enum.Pid.html)

В файле [`ku/src/process/pid.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/process/pid.rs) описана структура

```rust
{{#include ../../ku/src/process/pid.rs:pid}}
```

Она позволяет отдельно указывать либо
[`Pid::Current`](../../doc/ku/process/pid/enum.Pid.html#variant.Current) ---
текущий процесс, это удобно для использования тех системных вызовов, что принимают на вход
[`Pid`](../../doc/ku/process/pid/enum.Pid.html).
Либо конкретный процесс
[`Pid::Id`](../../doc/ku/process/pid/enum.Pid.html#variant.Id),
идентификатор которого состоит из номера слота в таблице процессов
[`slot`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.slot)
и эпохи этого слота
[`epoch`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.epoch).
Поле [`slot`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.slot) позволяет быстро находить процесс по его идентификатору в таблице процессов, ---
она может быть устроена как вектор, а не как хеш-таблица.
А [`epoch`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.epoch) позволяет сделать идентификаторы процессов уникальными на протяжении всего времени работы системы.

Например, в Unix есть гонка, из-за того что одно и то же значение `pid_t` может в разное время ссылаться на разные процессы.
Если вы хотите что-то сделать с определённым процессом, и записали его `pid_t` себе,
то конкурентно он может прекратить своё исполнение, а ядро выдаст то же самое значение `pid_t` новому процессу.
Теперь вы что-то делаете с процессом по сохранённому у себя `pid_t`, например, посылаете ему сигнал `SIGKILL`, но это уже другой процесс.
Реализации пытаются уменьшить вероятность таких гонок за счёт выдачи `pid_t` по кругу.

> В linux недавно появился ещё один способ избежать такую гонку.
> [С процессом сопоставляется файловый дескриптор](https://lwn.net/Articles/794707/),
> через который можно послать сигнал или дождаться завершения процесса.

В Nikka для избежания гонки, каждый раз, когда новый процесс получает тот же
[`slot`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.slot),
он получает на единицу большее значение
[`epoch`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.epoch).
За это отвечает метод
[`Pid::next_epoch()`](../../doc/ku/process/pid/enum.Pid.html#method.next_epoch).

Другие методы [`Pid`](../../doc/ku/process/pid/enum.Pid.html):

- [`Pid::new(slot)`](../../doc/ku/process/pid/enum.Pid.html#method.new) создаёт [`Pid`](../../doc/ku/process/pid/enum.Pid.html) с начальным значением [`epoch`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.epoch) для заданного `slot`.
- [`Pid::slot()`](../../doc/ku/process/pid/enum.Pid.html#method.slot) возвращает значение [`slot`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.slot).
- [`Pid::into_usize()`](../../doc/ku/process/pid/enum.Pid.html#method.into_usize) и [`Pid::from_usize()`](../../doc/ku/process/pid/enum.Pid.html#method.from_usize) позволяют сериализовать [`Pid`](../../doc/ku/process/pid/enum.Pid.html) в регистр для передачи в системные вызовы.


### Слот таблицы процессов [`kernel::process::table::Slot`](../../doc/kernel/process/table/enum.Slot.html)

Таблица процессов состоит из фиксированного количества слотов

```rust
enum Slot {
    Free {
        pid: Pid,
        next: Option<Pid>,
    },
    Occupied {
        process: Spinlock<Process>,
    },
}
```

Каждый из которых либо свободен ---
[`Slot::Free`](../../doc/kernel/process/table/enum.Slot.html#variant.Free),
либо занят ---
[`Slot::Occupied`](../../doc/kernel/process/table/enum.Slot.html#variant.Occupied).

Свободные слоты содержат поле
[`pid`](../../doc/kernel/process/table/enum.Slot.html#variant.Free.field.pid)
для того, что помнить эпоху последнего процесса, занимавшего этот слот.
И выдать следующему процессу, который попадёт в тот же слот, номер эпохи на единицу больше.
Также свободные слоты провязаны в список, чтобы удобнее было за \\(O(1)\\)
находить какой-нибудь свободный слот под новый процесс.

Занятые слоты содержат спин–блокировку
[`process`](../../doc/kernel/process/table/enum.Slot.html#variant.Occupied.field.process).
Когда нам понадобится обратиться к процессу, нужно будет захватить его спин–блокировку,
чтобы избежать неконсистентного конкурентного изменения структуры
[`kernel::process::process::Process`](../../doc/kernel/process/process/struct.Process.html).
Чтобы обратиться к самой таблице процессов
[`static ref kernel::process::table::TABLE: Spinlock<Table>`](../../doc/kernel/process/table/struct.TABLE.html),
конечно тоже нужно захватить её общую спин–блокировку.
Но не хочется держать заблокированной спин–блокировку всей таблицы, пока мы работаем со структурой
[`kernel::process::process::Process`](../../doc/kernel/process/process/struct.Process.html)
одного её процесса.
Поэтому, таблица будет по идентификатору процесса
[`Pid`](../../doc/ku/process/pid/enum.Pid.html)
фактически обменивать заблокированную спин–блокировку `Spinlock<Table>` на заблокированную спин–блокировку процесса `Spinlock<Process>`.
После чего вызывающий код сможет работать с процессом конкурентно другому коду,
который сможет захватить уже освободившийся `Spinlock<Table>`.


### Задача 5 --- таблица процессов


#### Инициализация таблицы

Реализуйте [метод](../../doc/kernel/process/struct.Table.html#method.new)

```rust
fn Table::new(len: usize) -> Self
```

в файле
[`kernel/src/process/table.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/table.rs).

Он создаёт таблицу процессов
[`Table::table`](../../doc/kernel/process/struct.Table.html#structfield.table)
размера `len` элементов, заполняя её пустыми слотами
[`Slot::Free`](../../doc/kernel/process/table/enum.Slot.html#variant.Free)
с соответствующими индексам слотов полями
[`Pid::Id::slot`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.slot).
Эти пустые слоты он провязывает в односвязный список с головой в поле
[`Table::free`](../../doc/kernel/process/struct.Table.html#structfield.free).
Чтобы избежать переаллокаций, рекомендуется использовать метод
[`alloc::vec::Vec::with_capacity()`](https://doc.rust-lang.org/nightly/alloc/vec/struct.Vec.html#method.with_capacity).


#### Аллокация слота под процесс

Реализуйте [метод](../../doc/kernel/process/struct.Table.html#method.allocate)

```rust
fn Table::allocate(mut process: Process) -> Result<Pid>
```

Он должен выделить новому процессу `process` свободный слот таблицы.
Слот возьмите из головы списка свободных
[`Table::free`](../../doc/kernel/process/struct.Table.html#structfield.free).
Запишите в него `process`, а
[`pid`](../../doc/kernel/process/table/enum.Slot.html#variant.Free.field.pid)
слота запишите в структуру процесса `process` методом
[`Process::set_pid()`](../../doc/kernel/process/process/struct.Process.html#method.set_pid).
Если же свободного слота нет, верните ошибку
[`Error::NoProcessSlot`](../../doc/kernel/error/enum.Error.html#variant.NoProcessSlot).
В этом случае при выходе из метода, `process` будет автоматически уничтожен, а все его ресурсы освобождены.
Так как по сигнатуре
[`Table::allocate(process: Process)`](../../doc/kernel/process/struct.Table.html#method.allocate)
поглощает свой аргумент.
Этот и все последующие методы --- статические, они оперируют с глобальным
[синглтоном](https://en.wikipedia.org/wiki/Singleton_pattern)
[`static ref TABLE: Spinlock<Table>`](../../doc/kernel/process/table/struct.TABLE.html),
захватывая его блокировку --- `TABLE.lock()`.
При реализации вам может пригодиться метод
[`Option::take()`](https://doc.rust-lang.org/nightly/core/option/enum.Option.html#method.take).


#### Получение процесса по его идентификатору

Реализуйте [метод](../../doc/kernel/process/struct.Table.html#method.get)

```rust
fn Table::get(pid: Pid) -> Result<SpinlockGuard<'static, Process>>
```

Он возвращает заблокированную спин–блокировку
[`ku::sync::SpinlockGuard`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html)
со структурой
[`kernel::process::process::Process`](../../doc/kernel/process/process/struct.Process.html),
соответствующей идентификатору `pid`.
Верните ошибку
[`Error::NoProcess`](../../doc/kernel/error/enum.Error.html#variant.NoProcess),
если процесса по указанному `pid` нет.
То есть, если либо не занят `pid.slot()`, либо в этом слоте у процесса другое значение
[`kernel::process::process::Process::pid()`](../../doc/kernel/process/process/struct.Process.html#method.pid)
(значит у него другая эпоха
[`Pid::epoch`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.epoch),
так как слот
[`Pid::slot`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.slot)
должен совпадать).
Вытащить значение
[`epoch`](../../doc/ku/process/pid/enum.Pid.html#variant.Id.field.epoch) тип
[`Pid`](../../doc/ku/process/pid/enum.Pid.html)
не позволяет, но зато он позволяет сравнивать два своих значения на равенство за счёт реализации
`#[derive(..., Eq, PartialEq)]`.

Так как размер таблицы процессов
[`static ref TABLE: Spinlock<Table>`](../../doc/kernel/process/table/struct.TABLE.html)
после инициализации мы никогда не меняем, и в частности не уменьшаем,
время жизни каждого её слота --- практически `'static`.
Который и указан в результирующем типе метода
[`Table::get()`](../../doc/kernel/process/struct.Table.html#method.get).
Но Rust не может проверить это самостоятельно.
Нам придётся пообещать ему это с помощью
[unsafe--функции](../../doc/kernel/process/table/fn.forge_static_lifetime.html)

```rust
unsafe fn forge_static_lifetime<T>(x: &T) -> &'static T
```

Естественно, чтобы вернуть
[`SpinlockGuard`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html),
нужно заблокировать процесс в слоте методом
[`Spinlock::lock()`](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.lock).
Спин–блокировка же самой
[`TABLE`](../../doc/kernel/process/table/struct.TABLE.html)
будет автоматически разблокирована аналогичным гардом при выходе из функции.
Получается, что она в начале захватывает низкогранулярную блокировку на всю таблицу
[`TABLE`](../../doc/kernel/process/table/struct.TABLE.html),
а потом повышает гранулярность этой блокировки до блокировки одного слота таблицы.
И вызывающая функция в дальнейшем работает уже с высокогранулярной блокировкой.


#### Освобождение слота с уничтожением процесса

Реализуйте [метод](../../doc/kernel/process/struct.Table.html#method.free)

``` rust
fn Table::free(mut pid: Pid) -> Result<()>
```

Он:

- Удаляет процесс с заданным `pid`.
- Инкрементирует эпоху в освободившемся слоте.
- Вставляет слот в голову списка свободных слотов [`Table::free`](../../doc/kernel/process/struct.Table.html#structfield.free).


### Проверьте себя

Теперь должны заработать тесты `basic()` и `full_capacity()` в файле
[`kernel/tests/4-process-5-table.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-5-table.rs):

```console
$ (cd kernel; cargo test --test 4-process-5-table)
...
4_process_5_table::basic------------------------------------
20:37:39 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:37:39 0 I duplicate; address_space = "process" @ 0p7E2_6000
20:37:39 0 I switch to; address_space = "process" @ 0p7E2_6000
20:37:39 0 I switch to; address_space = "base" @ 0p1000
20:37:39 0 I allocate; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_6000, { rip: 0v0, rsp: 0v0 } }; process_count = 1
20:37:40 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:37:40 0 I duplicate; address_space = "process" @ 0p7E0_B000
20:37:40 0 I switch to; address_space = "process" @ 0p7E0_B000
20:37:40 0 I switch to; address_space = "base" @ 0p1000
20:37:40 0 I allocate; slot = Process { pid: 1:0, address_space: "1:0" @ 0p7E0_B000, { rip: 0v0, rsp: 0v0 } }; process_count = 2
20:37:40 0 D pid_1 = 0:0; pid_2 = 1:0
20:37:40 0 I free; slot = Process { pid: 0:0, address_space: "0:0" @ 0p7E2_6000, { rip: 0v0, rsp: 0v0 } }; process_count = 1
20:37:40 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:37:40 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 2, unlocks: 2, waits: 0 }
20:37:40 0 I drop; address_space = "0:0" @ 0p7E2_6000
20:37:40 0 I free; slot = Process { pid: 1:0, address_space: "1:0" @ 0p7E0_B000, { rip: 0v0, rsp: 0v0 } }; process_count = 0
20:37:40 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:37:40 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 2, unlocks: 2, waits: 0 }
20:37:40 0 I drop; address_space = "1:0" @ 0p7E0_B000
4_process_5_table::basic--------------------------- [passed]

4_process_5_table::full_capacity----------------------------
20:37:40 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:37:40 0 I duplicate; address_space = "process" @ 0p7E0_B000
20:37:40 0 I switch to; address_space = "process" @ 0p7E0_B000
20:37:40 0 I switch to; address_space = "base" @ 0p1000
20:37:40 0 I allocate; slot = Process { pid: 1:1, address_space: "1:1" @ 0p7E0_B000, { rip: 0v0, rsp: 0v0 } }; process_count = 1
20:37:41.069 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:37:41.075 0 I duplicate; address_space = "process" @ 0p7E2_6000
20:37:41.079 0 I switch to; address_space = "process" @ 0p7E2_6000
20:37:41.085 0 I switch to; address_space = "base" @ 0p1000
20:37:41.089 0 I allocate; slot = Process { pid: 0:1, address_space: "0:1" @ 0p7E2_6000, { rip: 0v0, rsp: 0v0 } }; process_count = 2
20:37:41.283 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:37:41.291 0 I duplicate; address_space = "process" @ 0p7DF_0000
20:37:41.295 0 I switch to; address_space = "process" @ 0p7DF_0000
20:37:41.301 0 I switch to; address_space = "base" @ 0p1000
20:37:41.305 0 I allocate; slot = Process { pid: 2:0, address_space: "2:0" @ 0p7DF_0000, { rip: 0v0, rsp: 0v0 } }; process_count = 3
20:37:41.497 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
...
20:38:38.947 0 I duplicate; address_space = "process" @ 0p632_6000
20:38:38.951 0 I switch to; address_space = "process" @ 0p632_6000
20:38:38.957 0 I switch to; address_space = "base" @ 0p1000
20:38:38.961 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 0, unlocks: 0, waits: 0 }
20:38:38.969 0 I drop; address_space = "process" @ 0p632_6000
20:38:39.283 0 D prev_pid = 1:2; pid = 1:3
20:38:39.287 0 I free; slot = Process { pid: 1:3, address_space: "1:3" @ 0p7E0_B000, { rip: 0v0, rsp: 0v0 } }; process_count = 255
20:38:39.295 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:38:39.301 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 2, unlocks: 2, waits: 0 }
20:38:39.307 0 I drop; address_space = "1:3" @ 0p7E0_B000
20:38:39.731 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:38:39.739 0 I duplicate; address_space = "process" @ 0p7E0_B000
20:38:39.743 0 I switch to; address_space = "process" @ 0p7E0_B000
20:38:39.751 0 I switch to; address_space = "base" @ 0p1000
20:38:39.755 0 I allocate; slot = Process { pid: 1:4, address_space: "1:4" @ 0p7E0_B000, { rip: 0v0, rsp: 0v0 } }; process_count = 256
20:38:39.965 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:38:39.973 0 I duplicate; address_space = "process" @ 0p632_6000
20:38:39.977 0 I switch to; address_space = "process" @ 0p632_6000
20:38:39.983 0 I switch to; address_space = "base" @ 0p1000
20:38:39.987 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 0, unlocks: 0, waits: 0 }
20:38:39.995 0 I drop; address_space = "process" @ 0p632_6000
20:38:40.301 0 D prev_pid = 1:3; pid = 1:4
...
20:39:48.755 0 I free; slot = Process { pid: 2:0, address_space: "2:0" @ 0p7DF_0000, { rip: 0v0, rsp: 0v0 } }; process_count = 2
20:39:48.761 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:39:48.769 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 2, unlocks: 2, waits: 0 }
20:39:48.775 0 I drop; address_space = "2:0" @ 0p7DF_0000
20:39:49.051 0 I free; slot = Process { pid: 0:1, address_space: "0:1" @ 0p7E2_6000, { rip: 0v0, rsp: 0v0 } }; process_count = 1
20:39:49.063 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:39:49.071 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 2, unlocks: 2, waits: 0 }
20:39:49.079 0 I drop; address_space = "0:1" @ 0p7E2_6000
20:39:49.313 0 I free; slot = Process { pid: 1:11, address_space: "1:11" @ 0p7E0_B000, { rip: 0v0, rsp: 0v0 } }; process_count = 0
20:39:49.321 0 D dropping; spinlock = kernel/src/process/table.rs:127:26; stats = Stats { failures: 0, locks: 3, unlocks: 3, waits: 0 }
20:39:49.327 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 2, unlocks: 2, waits: 0 }
20:39:49.335 0 I drop; address_space = "1:11" @ 0p7E0_B000
4_process_5_table::full_capacity------------------- [passed]
20:39:49.561 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/process/table.rs | 120 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++------
 1 file changed, 112 insertions(+), 8 deletions(-)
```
