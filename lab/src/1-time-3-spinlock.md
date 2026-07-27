## Спин–блокировка

Целью этой и последующей задачи является реализация
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


### Структура [`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)

В файле
[`ku/src/sync/spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/spinlock.rs)
определена структура
[`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html):

```rust
pub struct Spinlock<T> {
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
[`SpinlockGuard<'a, T>`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html)
параметризована защищаемым типом данных `T` и временем жизни `'a`
структуры
[`Spinlock<T>`](../../doc/ku/sync/spinlock/struct.Spinlock.html),
к которой она привязана.

В файле
[`ku/src/sync/spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/sync/spinlock.rs)
после выполнения задачи её определение должно содержать ссылку на исходный
[`Spinlock<T>`](../../doc/ku/sync/spinlock/struct.Spinlock.html):

```rust
pub struct SpinlockGuard<'a, T> {
    spinlock: &'a Spinlock<T>,
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


### Задача 2 --- реализация [`Spinlock`](../../doc/ku/sync/spinlock/struct.Spinlock.html)

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
) -> Option<SpinlockGuard<'_, T>>
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
fn Spinlock::try_lock(&self) -> Option<SpinlockGuard<'_, T>>
```

и публичный
[метод](../../doc/ku/sync/spinlock/struct.Spinlock.html#method.lock)

```rust
fn Spinlock::lock(&self) -> SpinlockGuard<'_, T>
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
[`&'a Spinlock<T>`](../../doc/ku/sync/spinlock/struct.Spinlock.html)
в определении структуры
[`SpinlockGuard`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html).
А в методах
[`SpinlockGuard::deref()`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html#method.deref)
и
[`SpinlockGuard::deref()`](../../doc/ku/sync/spinlock/struct.SpinlockGuard.html#method.deref_mut)
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


### Проверьте себя

На этот раз тесты находятся не в `kernel`, а в `ku` ---
[`ku/tests/1-time-2-spinlock.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-2-spinlock.rs).
Их можно запустить командой `cargo test --test 1-time-2-spinlock` в директории `ku` репозитория.
Вы увидите сборку и логи запуска тестов:

```console
/.../nikka$ (cd ku; cargo test --test 1-time-2-spinlock)
   Compiling ku v0.1.0 (/.../nikka/ku)
    Finished test [unoptimized + debuginfo] target(s) in 1.80s
     Running tests/1-time-2-spinlock.rs (/.../nikka/target/debug/deps/1_time_2_spinlock-9aabd4c2d0ab5e71)

running 6 tests
2023-07-22T16:40:07.457510Z : ThreadId(07) attempting create a deadlock on two spinlocks A=Spinlock { defined: ku/tests/1-time-2-spinlock.rs:97:31, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } } B=Spinlock { defined: ku/tests/1-time-2-spinlock.rs:98:31, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2023-07-22T16:40:07.457507Z : ThreadId(08) locked spinlock=Spinlock { defined: ku/tests/1-time-2-spinlock.rs:25:20, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2023-07-22T16:40:07.457508Z : ThreadId(09) unlocked spinlock=Spinlock { defined: ku/tests/1-time-2-spinlock.rs:66:24, locked: false, data: 1, stats: Stats { failures: 0, locks: 1, unlocks: 1, waits: 0 } }
2023-07-22T16:40:07.458180Z : ThreadId(09) dropping spinlock=ku/tests/1-time-2-spinlock.rs:66:24 stats=Stats { failures: 0, locks: 2, unlocks: 2, waits: 0 }
2023-07-22T16:40:07.458420Z : ThreadId(08) unlocked spinlock=Spinlock { defined: ku/tests/1-time-2-spinlock.rs:25:20, locked: false, data: 1, stats: Stats { failures: 2, locks: 2, unlocks: 2, waits: 2 } }
2023-07-22T16:40:07.458474Z : ThreadId(08) dropping spinlock=ku/tests/1-time-2-spinlock.rs:25:20 stats=Stats { failures: 2, locks: 3, unlocks: 3, waits: 2 }
test exclusive_access ... ok
test lock_unlock ... ok
2023-07-22T16:40:07.506380Z : ThreadId(22) acquired first lock lock_x=1
2023-07-22T16:40:07.506509Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.511349Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.530346Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.530439Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.530523Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.530606Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.530689Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.530773Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.530856Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.530938Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.531021Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.531103Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.531184Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.531266Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.531348Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.531429Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.531511Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.538944Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.539435Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.539525Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.539609Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.539692Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.539774Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.539856Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.539939Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.540021Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.540104Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.540187Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.540270Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.540353Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.540435Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.540517Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.540599Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.540682Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.540764Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.546345Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.546430Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.546514Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.546597Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.546679Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.546762Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.550371Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.550453Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.550535Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.550618Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.550701Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.550783Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.550866Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.550948Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551031Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551113Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551195Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551277Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551358Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551438Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551520Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551603Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551685Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551768Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551851Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.551933Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552016Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552098Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552181Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552263Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552345Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552428Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552510Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552592Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552675Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552758Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552840Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.552922Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553005Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553087Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553170Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553252Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553334Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553416Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553498Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553581Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553663Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553745Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.553828Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.558369Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.558453Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.558536Z : ThreadId(22) waiting on a barrier arrived=1
2023-07-22T16:40:07.570668Z : ThreadId(20) acquired first lock lock_x=0
2023-07-22T16:40:07.578410Z : ThreadId(20) waiting on a barrier arrived=2
2023-07-22T16:40:07.618351Z : ThreadId(22) waiting on a barrier arrived=2
2023-07-22T16:40:07.870405Z : ThreadId(115) locked spinlock=Spinlock { defined: ku/tests/1-time-2-spinlock.rs:47:20, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2023-07-22T16:40:07.870587Z : ThreadId(115) unlocked spinlock=Spinlock { defined: ku/tests/1-time-2-spinlock.rs:47:20, locked: false, data: 0, stats: Stats { failures: 2, locks: 2, unlocks: 2, waits: 2 } }
2023-07-22T16:40:07.870687Z : ThreadId(115) dropping spinlock=ku/tests/1-time-2-spinlock.rs:47:20 stats=Stats { failures: 2, locks: 3, unlocks: 3, waits: 2 }
test try_lock ... ok
2023-07-22T16:40:07.905846Z : ThreadId(114) attempting to lock a locked spinlock spinlock=Spinlock { defined: ku/tests/1-time-2-spinlock.rs:81:20, locked: true, stats: Stats { failures: 1, locks: 1, unlocks: 0, waits: 1 } }
2023-07-22T16:40:07.962697Z : ThreadId(07) ab_result=Err(Any { .. }) ba_result=Ok(())
test deadlock ... ok
2023-07-22T16:40:08.011447Z : ThreadId(114) dropping spinlock=ku/tests/1-time-2-spinlock.rs:81:20 stats=Stats { failures: 2, locks: 1, unlocks: 1, waits: 1000001 }
test recursive_deadlock - should panic ... ok
2023-07-22T16:40:08.759670Z : ThreadId(06) false positive deadlock detections false_positive_count=96
2023-07-22T16:40:08.759737Z : ThreadId(06) consistent_count=400000 inconsistent_count=0
2023-07-22T16:40:08.759774Z : ThreadId(06) spinlock=Spinlock { defined: ku/tests/1-time-2-spinlock.rs:157:49, locked: false, data: (400109, 800218), stats: Stats { failures: 96, locks: 400110, unlocks: 400110, waits: 104025504 } }
test concurrent ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.30s
```


### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/sync/spinlock.rs | 95 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++-----------------
 1 file changed, 77 insertions(+), 18 deletions(-)
```
