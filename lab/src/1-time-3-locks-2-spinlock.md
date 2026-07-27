## Спин–блокировка


### Структура [`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)

В файле
[`ku/src/sync/spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/spinlock.rs)
определена структура
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html):

```rust
pub struct Spinlock<T, const PANIC_STRATEGY> {
    data: UnsafeCell<T>,
    defined: &'static Location<'static>,
    locked: AtomicBool,
    owner: SequenceLock<Callsite>,
    stats: Stats,
}
```

Она параметризована произвольным типом `T`, который мы хотим защитить спин–блокировкой.
Чтобы получить доступ к значению этого типа, имея переменную типа
[`Spinlock<T>`](../../doc/ku/sync/spinlock/struct.Spinlock.html),
код должен будет захватить спин–блокировку и только после этого
сможет обратиться с содержащемуся внутри полю с защищаемыми данными типа `T`.
Без явного захвата блокировки обратиться с защищаемому полю не получится ---
это невозможно будет выразить в коде так, чтобы он скомпилировался.
Такой подход позволяет избавиться от ошибок из-за невнимательности.

Также она параметризована константой `PANIC_STRATEGY` типа
[`ku::sync::panic::PanicStrategy`](../../doc/ku/sync/panic/enum.PanicStrategy.html).
Она регулирует поведение конкретной спин–блокировки после паники ядра.

Обычно блокировки защищают какие-то данные от того чтобы они были прочитаны в некорректном состоянии.
Когда ядро паникует на одном из процессоров, оно может удерживать некоторые блокировки,
а данные могут находиться в неконсистентном состоянии --- могут быть нарушены какие-нибудь инварианты.
Поэтому лучшее, что можно сделать --- остановить ядро на всех процессорах,
чтобы не испортить ситуацию сильнее.
Момент попытки захвата блокировки вполне подходит для этой цели.
А реализуют её спин–блокировки у которых стратегия поведения при панике ---
[`PanicStrategy::Halt`](../../doc/ku/sync/panic/enum.PanicStrategy.html#variant.Halt).
Это значение по умолчанию для параметра `PANIC_STRATEGY`.

Но для блокировок, которые защищают механизмы логирования, этот вариант не удачен.
С одной стороны, в логировании нет ценных данных пользователя, которые можно было бы испортить.
А с другой стороны, пусть в чём-то некорректная, но отладочная информация о произошедшей панике
очень полезна при отладке.
Поэтому в блокировках, которые использует подсистема логирования, выбирается другая стратегия ---
[`PanicStrategy::KnockDown`](../../doc/ku/sync/panic/enum.PanicStrategy.html#variant.KnockDown).
При ней все блокировки становятся всё время открытыми, как пожарные выходы из зданий,
и позволяют захват произвольное конкурентное количество раз.
Обработчик паники сообщает блокировкам о том что ядро перешло в режим паники с помощью свободных функций
[`ku::sync::panic::start_panicing()`](../../doc/ku/sync/panic/fn.start_panicing.html) и
[`ku::sync::panic::is_panicing()`](../../doc/ku/sync/panic/fn.is_panicing.html).

Поле
[`Spinlock::data`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.data)
хранит защищаемые данные.
До выполнения этой задачи они обёрнуты в
[`spin::mutex::Mutex`](../../doc/spin/mutex/struct.Mutex.html),
а
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)
просто перекладывает всю свою работу на него.
Для выполнения задачи нужно будет заменить
[`spin::mutex::Mutex`](../../doc/spin/mutex/struct.Mutex.html)
на
[`core::cell::UnsafeCell`](https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html):

```rust
data: UnsafeCell<T>,
```

Структура
[`UnsafeCell<T>`](https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html)
обеспечивает только хранение данных типа `T`,
но не взаимное исключения при доступе к этим данным.
Им в свою очередь займётся
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html).

Поле
[`Spinlock::locked`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.locked)
определяет текущее состояние спин–блокировки.
Если оно равно `true`, то блокировка захвачена.

Остальные поля
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)
служат только для отладки.

- Поле
[`Spinlock::defined`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.defined)
содержит ссылку на строку кода, в которой была определена переменная типа
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html).

- Поле
[`Spinlock::owner`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.owner)
содержит информацию о месте кода, который последним успешно захватил спин–блокировку.
А именно, строку кода и ведущий к ней
[backtrace](../../lab/book/0-intro-5-gdb.html#%D0%91%D0%B5%D0%BA%D1%82%D1%80%D0%B5%D0%B9%D1%81).
Их хранит структура
[`ku::backtrace::callsite::Callsite`](../../doc/ku/backtrace/callsite/struct.Callsite.html).
Для того чтобы заполнить это поле корректно, нам понадобится выполнить следующую задачу.
А в этой задаче менять его не будем.

- Поле
[`Spinlock::stats`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.stats)
содержит статистики захвата блокировки
[`ku::sync::spinlock::Stats`](../../doc/ku/sync/spinlock/struct.Spinlock.html).
Иметь их приятно, но не обязательно. Поэтому тесты их не проверяют.


### Структура [`SpinlockGuard`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html)

Структура
[`SpinlockGuard<'a, T, PANIC_STRATEGY>`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html)
параметризована защищаемым типом данных `T` и временем жизни `'a`
структуры
[`Spinlock<T, PANIC_STRATEGY>`](../../doc/ku/sync/spinlock/struct.Spinlock.html),
к которой она привязана.

В файле
[`ku/src/sync/spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/spinlock.rs)
после выполнения задачи её определение должно содержать ссылку на исходный
[`Spinlock<T, PANIC_STRATEGY>`](../../doc/ku/sync/spinlock/struct.Spinlock.html):

```rust
pub struct SpinlockGuard<'a, T, const PANIC_STRATEGY> {
    spinlock: &'a Spinlock<T, PANIC_STRATEGY>,
}
```

Основной инвариант для
[`SpinlockGuard`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html)
такой: её собственное время жизни совпадает со временем удержания блокировки
[`SpinlockGuard::spinlock`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html#structfield.spinlock).
Поэтому создаётся она в методе
[`Spinlock::try_lock_impl()`](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.try_lock_impl),
который захватывает блокировку.
А при уничтожении
[`SpinlockGuard`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html)
в её реализации типажа
[`core::ops::Drop`](https://doc.rust-lang.org/nightly/core/ops/trait.Drop.html),
то есть в методе
[`SpinlockGuard::drop()`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html#method.drop),
блокировка соответствующего
[`SpinlockGuard::spinlock`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html#structfield.spinlock)
отпускается.
Это воплощение идиомы
[получение ресурса есть инициализация](https://en.wikipedia.org/wiki/Resource_acquisition_is_initialization)
(Resource Acquisition Is Initialization, RAII).
А метод
[`SpinlockGuard::drop()`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html#method.drop)
выполняет роль деструктора.


### Задача 3 --- реализация [`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)

Изучите [примеры использования `Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html#examples).

Поправьте тип поля
[`Spinlock::data`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.data)
на
[`UnsafeCell<T>`](https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html)
в определении структуры
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)
и в методе её создания
[`Spinlock::new()`](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.new).

Реализуйте приватный
[метод](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.try_lock_impl)

```rust
fn Spinlock::try_lock_impl(
    &self,
    max_tries: usize,
    callsite: Callsite,
) -> Option<SpinlockGuard<'_, T, PANIC_STRATEGY>>
```

в файле [`ku/src/sync/spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/spinlock.rs).

Этот метод пытается захватить спин–блокировку максимум `max_tries` раз.
Если за это количество попыток блокировка не освободилась,
возвращает
[`core::option::Option::None`](https://doc.rust-lang.org/nightly/core/option/enum.Option.html#variant.None).
Если же захватить блокировку получилось, то
[`Spinlock::try_lock_impl()`](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.try_lock_impl)
возвращает
[`core::option::Option::Some`](https://doc.rust-lang.org/nightly/core/option/enum.Option.html#variant.Some),
внутри которого находится гард
[`SpinlockGuard`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html).

При попытке захвата спин–блокировки потребуется атомарно проверить что она свободна,
то есть поле
[`Spinlock::locked`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.locked)
содержит `false` и одновременно поменять этот `false` на `true`.
Сделать это поможет

- метод
[`core::sync::atomic::AtomicBool::compare_exchange()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.compare_exchange) или
- метод
[`core::sync::atomic::AtomicBool::compare_exchange_weak()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak).

Внимательно расставьте
[`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html)
в вашей реализации.
На архитектуре
[x86-64](https://en.wikipedia.org/wiki/X86-64)
тесты не смогут проверить их корректность за вас.
Использовать аргумент `callsite` пока что не требуется.

Далее, с помощью реализованного метода
[`Spinlock::try_lock_impl()`](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.try_lock_impl),
реализуйте публичный
[метод](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.try_lock)

```rust
fn Spinlock::try_lock(&self) -> Option<SpinlockGuard<'_, T, PANIC_STRATEGY>>
```

и публичный
[метод](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.lock)

```rust
fn Spinlock::lock(&self) -> SpinlockGuard<'_, T, PANIC_STRATEGY>
```

в файле
[`ku/src/sync/spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/spinlock.rs).

Они отвечают за выбор значения `max_tries` и инициализацию

```rust
let callsite = Callsite::new(Location::caller());
```

с информацией о месте вызова, где блокировка захватывается.
Кроме того, метод
[`Spinlock::lock()`](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.lock)
должен запаниковать, если спин–блокировку захватить не удалось.
Для этого подойдёт макрос
[`core::panic!()`](https://doc.rust-lang.org/nightly/core/macro.panic.html).
В сообщение паники вставьте информацию о месте определения блокировки --- поле
[`Spinlock::defined`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.defined).
А также место неуспешного захвата блокировки --- `callsite`.
Нужно будет ещё добавить информацию о месте, откуда спин–блокировка была захвачена
в последний раз, но это отложим до реализации следующей задачи.

Поправьте тип поля
[`SpinlockGuard::spinlock`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html#structfield.spinlock)
на
[`&'a Spinlock<T, PANIC_STRATEGY>`](../../doc/ku/sync/spinlock/struct.Spinlock.html)
в определении структуры
[`SpinlockGuard`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html).
А в методах
[`SpinlockGuard::deref()`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html#method.deref)
и
[`SpinlockGuard::deref_mut()`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html#method.deref_mut)
замените `self.spinlock.deref()` и `self.spinlock.deref_mut()`
на

```rust
unsafe { &*self.spinlock.data.get() }
```

и

```rust
unsafe { &mut *self.spinlock.data.get() }
```

соответственно.
Они позволяют получить неизменяемую и изменяемую ссылки на содержащиеся в
[`Spinlock::data`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.data)
данные.
Для этого используется метод
[`core::cell::UnsafeCell::get()`](https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html#method.get),
который возвращает указатель на эти данные.

Разыменовать его можно только в `unsafe`--коде.
По сигнатуре
[`UnsafeCell::get(&self) -> *mut T`](https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html#method.get)
принимает неизменяемую --- неэксклюзивную --- ссылку `&self`.
Поэтому компилятор не может проверить,
что доступ по полученному указателю `*mut T` эксклюзивен в случае операций записи.
В данном случае ключевое слово `unsafe` --- наше обещание компилятору, что это так.
Обеспечивает его алгоритм спин–блокировки.
А проверяют уже тесты, а не компилятор.

Реализуйте
[метод](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html#method.drop)

```rust
fn SpinlockGuard::drop(&mut self)
```

в файле
[`ku/src/sync/spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/spinlock.rs).
Он должен освободить захваченную блокировку.

Также предлагается доработать реализацию типажа
[`core::fmt::Debug`](https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html)
для
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html).
А именно, распечатать захвачена ли спин–блокировка в данный момент --- поле
[`Spinlock::locked`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.locked).
В случае, если спин–блокировка свободна, стоит также распечатать текущее значение защищаемых ею данных, --- поле
[`Spinlock::data`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.data).
Если же она захвачена, стоит распечатать текущего владельца блокировки --- поле
[`Spinlock::owner`](../../doc/ku/sync/spinlock/struct.Spinlock.html#structfield.owner),
но это отложим до реализации следующей задачи.

При решении этой задачи вам могут пригодиться:

- Метод [`core::sync::atomic::AtomicBool::compare_exchange()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.compare_exchange).
- Метод [`core::sync::atomic::AtomicBool::compare_exchange_weak()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.compare_exchange_weak).
- Метод [`core::sync::atomic::AtomicBool::load()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.load).
- Метод [`core::sync::atomic::AtomicBool::store()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicBool.html#method.store).
- Метод [`core::sync::atomic::AtomicUsize::fetch_add()`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicUsize.html#method.fetch_add)у
- Функция [`core::hint::spin_loop()`](https://doc.rust-lang.org/nightly/core/hint/fn.spin_loop.html), которая сообщает процессору, что он находится в цикле активного ожидания внешнего события.

> В своей реализации вы можете применить какие-нибудь из
> [стандартных оптимизаций](https://en.wikipedia.org/wiki/Spinlock#Significant_optimizations).
> При этом имеет смысл оптимизировать только случай, когда блокировка захватывается с первого раза.
> Оптимизировать цикл ожидания её освобождения не имеет смысла --- то, когда этот цикл закончится,
> зависит не от оптимальности его кода, а от того когда блокировку освободит конкурентный код.
> Но в этом случае можно сэкономить электроэнергию, возможно, заряд аккумулятора.
> Для этого мы и используем
> [`core::hint::spin_loop()`](https://doc.rust-lang.org/nightly/core/hint/fn.spin_loop.html).


### Проверьте себя

#### Обычный запуск теста [`ku/tests/1-time-3-spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-3-spinlock.rs)

На этот раз тесты находятся не в `kernel`, а в `ku` ---
[`ku/tests/1-time-3-spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-3-spinlock.rs).
Их можно запустить командой `cargo test --test 1-time-3-spinlock` в директории `ku` репозитория.
Вы увидите сборку и логи запуска тестов:

```console
/.../nikka$ (cd ku; cargo test --test 1-time-3-spinlock -- --test-threads=1)
...
running 5 tests
test concurrent ... 2024-09-15T18:40:44.020212Z : concurrent false positive deadlock detections false_positive_count=96
2024-09-15T18:40:44.020413Z : concurrent consistent=400000 inconsistent=0
2024-09-15T18:40:44.020477Z : concurrent spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:146:29, locked: false, data: (400037, 800074), stats: Stats { failures: 96, locks: 400038, unlocks: 400038, waits: 112625451 } }
2024-09-15T18:40:44.020552Z : concurrent dropping spinlock=ku/tests/1-time-3-spinlock.rs:146:29 stats=Stats { failures: 96, locks: 400038, unlocks: 400038, waits: 112625451 }
ok
test deadlock ... 2024-09-15T18:40:44.021420Z :   deadlock attempting create a deadlock on two spinlocks a=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:81:22, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } } b=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:82:22, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2024-09-15T18:40:44.021626Z : ThreadId(106) acquired first lock lock_x=0
2024-09-15T18:40:44.021655Z : ThreadId(107) acquired first lock lock_x=1
2024-09-15T18:40:44.021698Z : ThreadId(106) waiting on a barrier arrived=1
2024-09-15T18:40:44.021723Z : ThreadId(107) waiting on a barrier arrived=2
2024-09-15T18:40:44.021744Z : ThreadId(106) waiting on a barrier arrived=2
2024-09-15T18:40:44.045714Z :   deadlock ab_result=Err(Any { .. }) ba_result=Err(Any { .. })
2024-09-15T18:40:44.045853Z :   deadlock dropping spinlock=ku/tests/1-time-3-spinlock.rs:82:22 stats=Stats { failures: 2, locks: 2, unlocks: 2, waits: 1000001 }
2024-09-15T18:40:44.045967Z :   deadlock dropping spinlock=ku/tests/1-time-3-spinlock.rs:81:22 stats=Stats { failures: 2, locks: 2, unlocks: 2, waits: 1000001 }
ok
test exclusive_access ... 2024-09-15T18:40:44.046349Z : exclusive_access unlocked spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:66:24, locked: false, data: 1, stats: Stats { failures: 0, locks: 1, unlocks: 1, waits: 0 } }
2024-09-15T18:40:44.046442Z : exclusive_access dropping spinlock=ku/tests/1-time-3-spinlock.rs:66:24 stats=Stats { failures: 0, locks: 2, unlocks: 2, waits: 0 }
ok
test lock_unlock ... 2024-09-15T18:40:44.046751Z :      lock_unlock locked spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:25:20, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2024-09-15T18:40:44.046866Z :      lock_unlock unlocked spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:25:20, locked: false, data: 1, stats: Stats { failures: 2, locks: 2, unlocks: 2, waits: 2 } }
2024-09-15T18:40:44.046954Z :      lock_unlock dropping spinlock=ku/tests/1-time-3-spinlock.rs:25:20 stats=Stats { failures: 2, locks: 3, unlocks: 3, waits: 2 }
ok
test try_lock ... 2024-09-15T18:40:44.047226Z :         try_lock locked spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:47:20, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2024-09-15T18:40:44.047301Z :         try_lock unlocked spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:47:20, locked: false, data: 0, stats: Stats { failures: 2, locks: 2, unlocks: 2, waits: 2 } }
2024-09-15T18:40:44.047370Z :         try_lock dropping spinlock=ku/tests/1-time-3-spinlock.rs:47:20 stats=Stats { failures: 2, locks: 3, unlocks: 3, waits: 2 }
ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.49s
```


#### Запуск теста [`ku/tests/1-time-3-spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-3-spinlock.rs) под [Miri](https://github.com/rust-lang/miri)

```console
/.../nikka$ (cd ku; cargo clean; MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test 1-time-3-spinlock)
...
running 4 tests
test concurrent ... 2024-09-15T18:42:22.765266Z : concurrent false positive deadlock detections false_positive_count=0
2024-09-15T18:42:22.937881Z : concurrent consistent=4999 inconsistent=0
2024-09-15T18:42:23.062438Z : concurrent spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:146:29, locked: false, data: (5000, 10000), stats: Stats { failures: 0, locks: 5001, unlocks: 5001, waits: 85673 } }
2024-09-15T18:42:23.233720Z : concurrent dropping spinlock=ku/tests/1-time-3-spinlock.rs:146:29 stats=Stats { failures: 0, locks: 5001, unlocks: 5001, waits: 85673 }
ok
test exclusive_access ... 2024-09-15T18:42:23.744910Z : exclusive_access unlocked spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:66:24, locked: false, data: 1, stats: Stats { failures: 0, locks: 1, unlocks: 1, waits: 0 } }
2024-09-15T18:42:23.926065Z : exclusive_access dropping spinlock=ku/tests/1-time-3-spinlock.rs:66:24 stats=Stats { failures: 0, locks: 2, unlocks: 2, waits: 8 }
ok
test lock_unlock ... 2024-09-15T18:42:24.412766Z :      lock_unlock locked spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:25:20, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2024-09-15T18:42:24.598041Z :      lock_unlock unlocked spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:25:20, locked: false, data: 1, stats: Stats { failures: 2, locks: 2, unlocks: 2, waits: 2 } }
2024-09-15T18:42:24.755148Z :      lock_unlock dropping spinlock=ku/tests/1-time-3-spinlock.rs:25:20 stats=Stats { failures: 2, locks: 3, unlocks: 3, waits: 2 }
ok
test try_lock ... 2024-09-15T18:42:25.297762Z :         try_lock locked spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:47:20, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 27 } }
2024-09-15T18:42:25.484351Z :         try_lock unlocked spinlock=Spinlock { defined: ku/tests/1-time-3-spinlock.rs:47:20, locked: false, data: 0, stats: Stats { failures: 2, locks: 2, unlocks: 2, waits: 28 } }
2024-09-15T18:42:25.639493Z :         try_lock dropping spinlock=ku/tests/1-time-3-spinlock.rs:47:20 stats=Stats { failures: 2, locks: 3, unlocks: 3, waits: 28 }
ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.37s
```


### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/sync/spinlock.rs | 95 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++-----------------
 1 file changed, 77 insertions(+), 18 deletions(-)
```
