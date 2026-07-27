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
[предыдущей задачи](../../lab/book/1-time-3-locks-3-sequence-lock.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-3--%D1%80%D0%B5%D0%B0%D0%BB%D0%B8%D0%B7%D0%B0%D1%86%D0%B8%D1%8F-sequencelock).
А в чём-то он будет сложнее, --- теперь нам не доступен
[приём read-don't-modify-write](https://web.archive.org/web/20230318132751/https://www.hpl.hp.com/techreports/2012/HPL-2012-68.pdf).
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
[предыдущей задаче](../../lab/book/1-time-3-locks-3-sequence-lock.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-3--%D1%80%D0%B5%D0%B0%D0%BB%D0%B8%D0%B7%D0%B0%D1%86%D0%B8%D1%8F-sequencelock).


### Задача 5 --- реализация [`AtomicCorrelationPoint`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html)

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
[приём "read-dont-modify-write"](https://web.archive.org/web/20230318132751/https://www.hpl.hp.com/techreports/2012/HPL-2012-68.pdf).
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
[приём read-don't-modify-write](https://web.archive.org/web/20230318132751/https://www.hpl.hp.com/techreports/2012/HPL-2012-68.pdf)
теперь придётся придумать более хитрую схему синхронизации, чем в
[предыдущей задачи](../../lab/book/1-time-3-locks-3-sequence-lock.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-3--%D1%80%D0%B5%D0%B0%D0%BB%D0%B8%D0%B7%D0%B0%D1%86%D0%B8%D1%8F-sequencelock).

Тесты проверяют корректность расстановки
[`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html)
с помощью эмулятора
[Miri](https://github.com/rust-lang/miri).
[Как и раньше](../../lab/book/1-time-3-locks-2-spinlock.html#%D0%9C%D0%BE%D0%B4%D0%B5%D0%BB%D1%8C-%D0%BF%D0%B0%D0%BC%D1%8F%D1%82%D0%B8-acquire-%D0%B8-release),
[`Ordering::SeqCst`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html#variant.SeqCst)
использовать не рекомендуется.

Вы можете опираться на тот факт, что писатель один и методы записи ---
[`AtomicCorrelationPoint::store()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.store) и
[`AtomicCorrelationPoint::inc()`](../../doc/ku/time/correlation_point/struct.AtomicCorrelationPoint.html#method.inc) ---
гарантированно не вызываются конкурентно ни каждый сам с собой, ни друг с другом.


### Запуск тестов

Тесты можно запустить командой `cargo test --test 1-time-5-correlation-point`
в директориях `ku` и `kernel` репозитория.
В проверочной системе запускаются тесты из обеих этих директорий.


#### Обычный запуск теста [`ku/tests/1-time-5-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-5-correlation-point.rs)

Тест, который находится в файле
[`ku/tests/1-time-5-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-5-correlation-point.rs),
похож на тест из предыдущей задачи.
Это стресс--тест с писателем и читателем, который запускается в многопоточном окружении под linux:

```console
/.../nikka$ (cd ku; cargo test --test 1-time-5-correlation-point -- --test-threads=1)
...
running 3 tests
test atomic_correlation_point ... ok
test correlation_point ... ok
test single_writer ... 2024-09-15T19:05:05.412813Z : single_writer consistent=13813664 inconsistent=0
ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.49s
```


#### Запуск теста [`ku/tests/1-time-5-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/1-time-5-correlation-point.rs) под [Miri](https://github.com/rust-lang/miri)

Запустите тест под
[Miri](https://github.com/rust-lang/miri),
дополнительно передав ему параметр `MIRIFLAGS="-Zmiri-disable-isolation"`:

```console
/.../nikka$ (cd ku; MIRIFLAGS="-Zmiri-disable-isolation" cargo miri test --test 1-time-5-correlation-point)
...
running 3 tests
test atomic_correlation_point ... ok
test correlation_point ... ok
test single_writer ... 2024-09-15T19:06:54.568237Z : single_writer consistent=19543 inconsistent=0
ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.93s
```


#### Запуск тестов из файла [`kernel/tests/1-time-5-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/1-time-5-correlation-point.rs)

Тесты в файле
[`kernel/tests/1-time-5-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/1-time-5-correlation-point.rs)
запускаются в режиме ядра и используют прерывания для запуска конкурентного кода.

```console
/.../nikka$ (cd kernel; cargo test --test 1-time-5-correlation-point)
...
Running: `qemu-system-x86_64 -drive format=raw,file=/.../nikka/target/kernel/debug/deps/bootimage-1_time_5_correlation_point-72dcb329204c56d5.bin -no-reboot -m size=128M -smp cpus=4 -device isa-debug-exit,iobase=0xF4,iosize=0x04 -serial stdio -display none -drive file=../fs.img,index=1,media=disk,driver=raw`
19:09:33 0 I RTC init; acknowledged_settings = RegisterB(USE_24_HOUR_FORMAT | UPDATE_ENDED_INTERRUPT)
19:09:33 0 I time init
19:09:33 0 I Nikka booted; now = 2024-09-15 19:09:33 UTC; tsc = Tsc(3388378113)
19:09:33 0 I GDT init
19:09:33 0 I traps init
running 2 tests

1_time_5_correlation_point::correlation_point_reader--------
19:09:35 0 D same = 720; different = 364; point = CorrelationPoint { count: 726000, tsc: 1452000 }
19:09:36 0 D same = 1469; different = 743; point = CorrelationPoint { count: 1485000, tsc: 2970000 }
19:09:37 0 D same = 2223; different = 1124; point = CorrelationPoint { count: 2246000, tsc: 4492000 }
19:09:38 0 D same = 2978; different = 1506; point = CorrelationPoint { count: 3010000, tsc: 6020000 }
19:09:39 0 D same = 3731; different = 1887; point = CorrelationPoint { count: 3773000, tsc: 7546000 }
19:09:40 0 D same = 4486; different = 2269; point = CorrelationPoint { count: 4537000, tsc: 9074000 }
19:09:41 0 D same = 5240; different = 2650; point = CorrelationPoint { count: 5298000, tsc: 10596000 }
19:09:42 0 D same = 5996; different = 3032; point = CorrelationPoint { count: 6062000, tsc: 12124000 }
19:09:43 0 D same = 6749; different = 3414; point = CorrelationPoint { count: 6826000, tsc: 13652000 }
19:09:44 0 D same = 7501; different = 3794; point = CorrelationPoint { count: 7586000, tsc: 15172000 }
19:09:45 0 D same = 8250; different = 4173; point = CorrelationPoint { count: 8345000, tsc: 16690000 }
19:09:46 0 D same = 9001; different = 4553; point = CorrelationPoint { count: 9105000, tsc: 18210000 }
19:09:47 0 D same = 9754; different = 4934; point = CorrelationPoint { count: 9866000, tsc: 19732000 }
19:09:48 0 D same = 10509; different = 5315; point = CorrelationPoint { count: 10629000, tsc: 21258000 }
19:09:49 0 D same = 11262; different = 5697; point = CorrelationPoint { count: 11393000, tsc: 22786000 }
19:09:50 0 D same = 12011; different = 6076; point = CorrelationPoint { count: 12150000, tsc: 24300000 }
19:09:51 0 D same = 12765; different = 6457; point = CorrelationPoint { count: 12913000, tsc: 25826000 }
19:09:52 0 D same = 13516; different = 6837; point = CorrelationPoint { count: 13673000, tsc: 27346000 }
19:09:53 0 D same = 14269; different = 7218; point = CorrelationPoint { count: 14434000, tsc: 28868000 }
19:09:54 0 D same = 15020; different = 7598; point = CorrelationPoint { count: 15194000, tsc: 30388000 }
19:09:55 0 D same = 15774; different = 7979; point = CorrelationPoint { count: 15957000, tsc: 31914000 }
19:09:56 0 D same = 16524; different = 8359; point = CorrelationPoint { count: 16717000, tsc: 33434000 }
19:09:57 0 D same = 17276; different = 8739; point = CorrelationPoint { count: 17477000, tsc: 34954000 }
19:09:58 0 D same = 18029; different = 9120; point = CorrelationPoint { count: 18238000, tsc: 36476000 }
19:09:59 0 D same = 18782; different = 9501; point = CorrelationPoint { count: 19001000, tsc: 38002000 }
19:10:00 0 D same = 19534; different = 9881; point = CorrelationPoint { count: 19761000, tsc: 39522000 }
1_time_5_correlation_point::correlation_point_reader [passed]

1_time_5_correlation_point::correlation_point_writer--------
19:10:00 0 D iteration = 0; failure_count = 0; success_count = 0; point = CorrelationPoint { count: 0, tsc: 0 }
19:10:01 0 D iteration = 824; failure_count = 123074; success_count = 223826; point = CorrelationPoint { count: 825, tsc: 1650 }
19:10:02 0 D iteration = 2063; failure_count = 307536; success_count = 559535; point = CorrelationPoint { count: 2063, tsc: 4126 }
19:10:03 0 D iteration = 3310; failure_count = 493488; success_count = 897522; point = CorrelationPoint { count: 3311, tsc: 6622 }
19:10:04 0 D iteration = 4557; failure_count = 679142; success_count = 1235249; point = CorrelationPoint { count: 4557, tsc: 9114 }
19:10:05 0 D iteration = 5303; failure_count = 784216; success_count = 1607382; point = CorrelationPoint { count: 5303, tsc: 10606 }
19:10:06 0 D iteration = 5784; failure_count = 846265; success_count = 2005302; point = CorrelationPoint { count: 5784, tsc: 11568 }
19:10:07 0 D iteration = 6262; failure_count = 907927; success_count = 2401540; point = CorrelationPoint { count: 6262, tsc: 12524 }
19:10:08 0 D iteration = 6742; failure_count = 969847; success_count = 2798852; point = CorrelationPoint { count: 6742, tsc: 13484 }
19:10:09 0 D iteration = 7222; failure_count = 1031767; success_count = 3196085; point = CorrelationPoint { count: 7222, tsc: 14444 }
19:10:10 0 D iteration = 7702; failure_count = 1093687; success_count = 3593388; point = CorrelationPoint { count: 7702, tsc: 15404 }
19:10:11 0 D iteration = 8183; failure_count = 1155865; success_count = 3992220; point = CorrelationPoint { count: 8184, tsc: 16368 }
19:10:12 0 D iteration = 8664; failure_count = 1217785; success_count = 4390456; point = CorrelationPoint { count: 8664, tsc: 17328 }
19:10:13 0 D iteration = 9146; failure_count = 1279963; success_count = 4789518; point = CorrelationPoint { count: 9146, tsc: 18292 }
19:10:14 0 D iteration = 9626; failure_count = 1341883; success_count = 5186924; point = CorrelationPoint { count: 9626, tsc: 19252 }
19:10:14 0 D iteration = 10000; failure_count = 1390129; success_count = 5495523
1_time_5_correlation_point::correlation_point_writer [passed]
19:10:14 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


> ##### Как устроен тест в [`kernel/tests/1-time-5-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/1-time-5-correlation-point.rs)
>
> В этой задаче дополнительные тесты в файле
> [`kernel/tests/1-time-5-correlation-point.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/1-time-5-correlation-point.rs)
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
> [обсудим чуть позже](../../lab/book/1-time-6-interrupts.html).
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
```rust
{{#include ../../kernel/tests/1-time-5-correlation-point.rs:down_the_rabbit_hole}}
```
>>
>> В отладочной сборке на логирование одной строки ушло 144218 инструкций процессора,
>> последняя из них располагалась по адресу `0v28_838E`.
>> А также было потрачено 4.203 KiB стека по адресам `[0v100_0200_0000, 0v100_0200_10D0)`
>> ```console
>> /.../nikka$ (cd kernel; cargo test --test 1-time-5-correlation-point)
>> ...
>> 1_time_5_correlation_point::down_the_rabbit_hole------------
>> 16:41:38 0 D waiting for the RTC to tick twice
>> 16:41:40 0 D how many instructions does it take to log something?; rflags = IF PF
>> 16:41:40 0 D how many instructions does it take to log something?; rflags = IF TF PF
>> 16:41:40 0 D instruction_count = 144218; used_stack_space = [0v100_0200_0000, 0v100_0200_10D0), size 4.203 KiB; last_traced_insruction_address = 0v28_838E
>> 16:41:40 0 D time_to_log_a_message = 5.422 ms; time_to_log_a_message_in_the_stepping_mode = 621.737 ms; stepping_slowdown_ratio = 114.67242158650633
>> 1_time_5_correlation_point::down_the_rabbit_hole--- [passed]
>> 16:41:40 0 I exit qemu; exit_code = ExitCode(SUCCESS)
>> ```
>> Возможно вы заметили, что второе сообщение `how many ...`, которое печаталось под трассировкой, появлялось на экране медленнее.
>>
>> В релизной сборке на логирование одной строки ушло 10503 инструкций и 1.000 KiB стека:
>>
>> ```console
>> /.../nikka$ (cd kernel; cargo test --test 1-time-5-correlation-point --release)
>> ...
>> 1_time_5_correlation_point::down_the_rabbit_hole------------
>> 16:45:53 0 D waiting for the RTC to tick twice
>> 16:45:55 0 D how many instructions does it take to log something?; rflags = IF
>> 16:45:55 0 D how many instructions does it take to log something?; rflags = IF TF
>> 16:45:55 0 D instruction_count = 10503; used_stack_space = [0v100_0200_1990, 0v100_0200_1D90), size 1.000 KiB; last_traced_insruction_address = 0v21_BD59
>> 16:45:55 0 D time_to_log_a_message = 1.654 ms; time_to_log_a_message_in_the_stepping_mode = 37.214 ms; stepping_slowdown_ratio = 22.494754670444543
>> 1_time_5_correlation_point::down_the_rabbit_hole--- [passed]
>> ```
>>
>>> Теперь вы можете, например, построить гистограмму количества раз, сколько исполнялись инструкции по разным адресам.
>>> И найти самые горячие циклы кода.


### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/time/correlation_point.rs | 55 +++++++++++++++++++++++++++++++++++++++++++++----------
 1 file changed, 45 insertions(+), 10 deletions(-)
```
