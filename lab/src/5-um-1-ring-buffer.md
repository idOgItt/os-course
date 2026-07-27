## Разделяемая память

Для общения между разными процессами и между кодом ядра и кодом режима пользователя внутри одного процесса необходим тот или иной метод
[межпроцессного взаимодействия](https://en.wikipedia.org/wiki/Inter-process_communication).
В Nikka используется [разделяемая память](https://en.wikipedia.org/wiki/Shared_memory),
организованная в виде [циклического буфера](https://en.wikipedia.org/wiki/Circular_buffer).
Для удобства в нём сделаны две доработки.


### Непрерывный циклический буфер

Обычный циклический буфер оборачивается вокруг своего конца, но адресовать память нужно линейно.
Поэтому если блок данных пересекает границу буфера, его нужно адресовать двумя частями ---
от начала блока данных до конца буфера и от начала буфера до конца блока данных.
Это иногда неудобно, и некоторые циклические буферы предоставляют метод для сдвига данных внутри буфера,
чтобы данные лежали последовательно.
Например, это делает метод
[`alloc::collections::vec_deque::VecDeque::make_contiguous()`](https://doc.rust-lang.org/nightly/alloc/collections/vec_deque/struct.VecDeque.html#method.make_contiguous).

Мы поступим [другим способом](https://fgiesen.wordpress.com/2012/07/21/the-magic-ring-buffer/).
Отобразим в виртуальную память буфер дважды подряд.
Тогда, при пересечении границы буфера, циклическое оборачивание адресов не нужно будет делать.
Точнее, за нас его сделает механизм виртуальной памяти.
В результате, любой отрезок буфера,
начинающийся по виртуальному адресу внутри первой копии буфера и не превышающий ёмкость буфера по размеру,
может быть использован как непрерывный.
Заплатим мы за это тем, что размер буфера должен быть кратен размеру страницы памяти.
А также двойным расходом виртуального адресного пространства и записей в кешах,
например в [TLB](https://wiki.osdev.org/TLB).

Аналогично сделано в библиотеке
[`magic_ring_buffer`](https://docs.rs/magic-ring-buffer/latest/magic_ring_buffer/).


### Транзакции

Добавим транзакции к интерфейсу буфера.
Каждая транзакция будет либо пишущей, либо читающей.

Пишущие транзакции будут удобны в следующей ситуации.
Мы будем писать логически атомарный блок данных по кусочкам.
И, возможно, он не влезет целиком.
А знать заранее полный размер мы не будем.
Тогда будет удобно сбросить транзакцию записи.
Кроме того, если исполнение кода переключится между записывающей и читающей сторонами,
читающая сторона не увидит в буфере только часть атомарного блока данных.

Для читающей стороны транзакции полезны другим.
Читающая сторона в некоторых случаях может не копировать данные из буфера к себе,
а обрабатывать данные прямо в буфере.
И отметить факт обработанности коммитом читающей транзакции, только когда ей эти данные больше не будут нужны.
Однако тут нужно быть осторожным.
Иногда читающая сторона не должна доверять пишущей, как в случае если читает код ядра, а пишет код пользователя.
Тогда она не должна, например, сначала провалидировать структуру данных в буфере, а после спокойно ей пользоваться.
Потому что пишущая сторона может подменить данные после валидации, но до их использования читающей стороной.
Ядро может препятствовать такой атаке за счёт того,
что не даст процессор коду пользователя во время работы читающей транзакции.
Другой вариант --- скопировать данные из буфера в собственную, не разделяемую, память.
А валидировать и использовать их уже там.


### Структура циклического буфера

В файле
[`ku/src/ring_buffer.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/ring_buffer.rs)
определён
[циклический буфер](https://en.wikipedia.org/wiki/Circular_buffer)
[`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html).

```rust
pub struct RingBuffer {
    block: Block<Page>,
    closed: bool,
    head: usize,
    tail: usize,
    stats: RingBufferStats,
}
```

ёмкости
[`RingBuffer::REAL_SIZE`](../../doc/ku/ring_buffer/struct.RingBuffer.html#associatedconstant.REAL_SIZE).
Его поля:

- [`RingBuffer::block`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.block) --- блок памяти с данными, которые хранятся в буфере.
- [`RingBuffer::closed`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.closed) --- закрыт ли буфер.
- [`RingBuffer::head`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.head) --- количество байт, прочитанных из буфера за всё время. То есть, эта величина потенциально больше размера буфера. Вариант хранить в `RingBuffer` это значение по модулю размера буфера, чреват ошибками.
- [`RingBuffer::tail`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.tail) --- количество байт, записанных в буфер за всё время.

Поле
[`RingBuffer::stats`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.stats)
поддерживает статистики чтения или записи в буфер:

```rust
pub struct RingBufferStats {
    commited: usize,
    commits: usize,
    dropped: usize,
    drops: usize,
    errors: usize,
    txs: usize,
}
```

- [`RingBufferStats::txs`](../../doc/ku/ring_buffer/struct.RingBufferStats.html#structfield.txs) --- количество транзакций чтения либо записи соответственно.
- [`RingBufferStats::commits`](../../doc/ku/ring_buffer/struct.RingBufferStats.html#structfield.commits) --- количество закоммиченных транзакций соответствующего типа.
- [`RingBufferStats::drops`](../../doc/ku/ring_buffer/struct.RingBufferStats.html#structfield.drops) --- количество оборванных (dropped, rolled back, aborted) транзакций соответствующего типа.
- [`RingBufferStats::commited`](../../doc/ku/ring_buffer/struct.RingBufferStats.html#structfield.commited) --- количество байт, прочитанных или записанных в закоммиченных транзакциях.
- [`RingBufferStats::dropped`](../../doc/ku/ring_buffer/struct.RingBufferStats.html#structfield.dropped) --- количество байт, прочитанных или записанных в оборванных транзакциях.
- [`RingBufferStats::errors`](../../doc/ku/ring_buffer/struct.RingBufferStats.html#structfield.errors) --- количество ошибок в транзакциях.

Для того чтобы при использовании буфера
[`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html)
не ошибиться и не попытаться записывать с читающего конца или наоборот,
заведены обёртки
[`ReadBuffer`](../../doc/ku/ring_buffer/type.ReadBuffer.html)
для читающего конца и
[`WriteBuffer`](../../doc/ku/ring_buffer/type.WriteBuffer.html)
дли пишущего.
Общий код же для избежания его дублирования помещён в сам
[`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html),
который сделан приватным.

Метод
[`RingBuffer::tx()`](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.tx)
создаёт транзакции:

```rust
fn RingBuffer::tx<T: Tag>(
    &mut self,
) -> Option<RingBufferTx<'_, T>>
```

Он обёрнут в публичные методы

- [`ReadBuffer::read_tx()`](../../doc/ku/ring_buffer/type.ReadBuffer.html#method.read_tx) для читающего конца.
- [`WriteBuffer::write_tx()`](../../doc/ku/ring_buffer/type.WriteBuffer.html#method.write_tx) --- для пишущего.

Сами транзакции
[`RingBufferTx`](../../doc/ku/ring_buffer/struct.RingBufferTx.html)
устроены так:

```rust
pub struct RingBufferTx<'a, T: Tag> {
    ring_buffer: &'a mut RingBuffer,
    header: usize,
    head: usize,
    tail: usize,
    bytes: usize,
    _tag: PhantomData<T>,
}
```

Они хранят:

- Ссылку [`RingBufferTx::ring_buffer`](../../doc/ku/ring_buffer/struct.RingBufferTx.html#structfield.ring_buffer) на исходный [`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html).
- [`RingBufferTx::header`](../../doc/ku/ring_buffer/struct.RingBufferTx.html#structfield.header) --- позиция в [`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html), по которому записан заголовок транзакции. Как и для остальных позиций в буфере, хранится до взятия по модулю.
- [`RingBufferTx::head`](../../doc/ku/ring_buffer/struct.RingBufferTx.html#structfield.head) и [`RingBufferTx::tail`](../../doc/ku/ring_buffer/struct.RingBufferTx.html#structfield.tail) --- актуальные в рамках транзакции значения, инициализирующиеся из полей [`RingBuffer::head`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.head) и [`RingBuffer::tail`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.tail) в момент старта транзакции.
- [`RingBufferTx::bytes`](../../doc/ku/ring_buffer/struct.RingBufferTx.html#structfield.bytes) --- количество байт, прочитанных или записанных на текущий момент в данной транзакции.
- Тег [`RingBufferTx::_tag`](../../doc/ku/ring_buffer/struct.RingBufferTx.html#structfield._tag), отличающий пишущие транзакции от читающих.

<!-- Типаж
[`ku::ring_buffer::Tag`](../../doc/ku/ring_buffer/trait.Tag.html)
служит для того чтобы отличать читающие транзакции от пишущих:

- [`ku::ring_buffer::ReadTag`](../../doc/ku/ring_buffer/struct.ReadTag.html) помечает читащие,
- [`ku::ring_buffer::WriteTag`](../../doc/ku/ring_buffer/struct.WriteTag.html) --- пишущие.
-->


### Разделяемые и не разделяемые данные

Обычно метаданные циклического буфера --- положение головы и хвоста --- разделяются
между читателем и писателем.
У нас это будет не так.
Разделяться будут только данные в буфере.
То есть, только то что находится в блоке памяти, описываемом
[`RingBuffer::block`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.block).
А сама структура
[`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html)
будет храниться в двух экземплярах --- по собственной копии у читающего и у пишущего концов.
По этой причине поля
[`RingBuffer::head`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.head)
и
[`RingBuffer::tail`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.tail)
имеют обычный тип
[`usize`](https://doc.rust-lang.org/nightly/core/primitive.usize.html),
а не атомарный
[`core::sync::atomic::AtomicUsize`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicUsize.html).

Информацию о своих текущих позициях в буфере читатель и писатель
будут передавать друг другу в заголовках транзакций.
Для сериализованного в буфере заголовка транзакции предлагается такая структура:

- Один байт для [`core::sync::atomic::AtomicU8`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicU8.html) --- состояния транзакции.
- Несколько байт для размера полезной нагрузки транзакции, размер заголовка тут предлагается не учитывать. Количество этих байт --- минимальное возможно для побайтной записи размеров от `0` до [`RingBuffer::REAL_SIZE`](../../doc/ku/ring_buffer/struct.RingBuffer.html#associatedconstant.REAL_SIZE).

Размер сериализованного заголовка задаётся константой
[`RingBuffer::HEADER_SIZE`](../../doc/ku/ring_buffer/struct.RingBuffer.html#associatedconstant.HEADER_SIZE),
а вычисляет его метод
[`RingBuffer::header_size()`](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.header_size).

В десериализованном виде заголовок описывает перечисление
[`ku::ring_buffer::Header`](../../doc/ku/ring_buffer/enum.Header.html).
Оно имеет такие варианты:

- [`Header::Clear`](../../doc/ku/ring_buffer/enum.Header.html#variant.Clear) --- транзакция свободна, то есть в неё ещё ничего не записывалось. Размер такой транзакции ещё не определён, то есть транзакция может простираться до конца свободного места в буфере.
- [`Header::Closed`](../../doc/ku/ring_buffer/enum.Header.html#variant.Closed) --- буфер закрыт, больше никаких данных через него передаваться не будет.
- [`Header::Locked`](../../doc/ku/ring_buffer/enum.Header.html#variant.Locked) --- транзакция захвачена писателем, но ещё не закоммичена или же отменена. Её размер на данный момент не известен.
- [`Header::Written { size }`](../../doc/ku/ring_buffer/enum.Header.html#variant.Written) --- транзакция закоммичена писателем, размер полезной нагрузки в транзакции равен `size`. При этом читатель возможно уже начал обрабатывать транзакцию, но ещё не закоммитил её.
- [`Header::Read { size }`](../../doc/ku/ring_buffer/enum.Header.html#variant.Read) --- транзакция закоммичена читателем, размер полезной нагрузки в транзакции равен `size`.

Этим вариантам соответствуют константы с
[`Header::CLEAR`](../../doc/ku/ring_buffer/enum.Header.html#associatedconstant.CLEAR)
по
[`Header::READ`](../../doc/ku/ring_buffer/enum.Header.html#associatedconstant.READ),
которые при сериализации записываются в
[`AtomicU8`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicU8.html)
состояния транзакции.


### Закрытие буфера

[Метод](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.close)

```rust
fn RingBuffer::close(
    &mut self,
)
```

закрывает буфер на чтение и запись.
Для упрощения, чтобы не реализовывать сложного протокола закрытия, он работает асимметрично.
Если буфер закрывает пишущая сторона, то читающая сначала прочитает все данные,
которые были записаны в буфер до закрытия.
Если же буфер закроет читающая сторона, то все данные, которые в него были записаны или
писались в этом момент конкурентно, будyт потеряны.

Закрывать буфер нужно в том числе при обнаружении нарушений инвариантов или протокола
взаимодействия противоположной стороной.
Поэтому не достаточно только записать
[`Header::Closed`](../../doc/ku/ring_buffer/enum.Header.html#variant.Closed)
в разделяюмую память,
противоположная сторона может затереть этот заголовок.
Запись
[`Header::Closed`](../../doc/ku/ring_buffer/enum.Header.html#variant.Closed)
предназначена только для информирования противоположной стороны.
Для себя нужно отдельно отметить факт закрытия буфера в не разделяюмой памяти ---
поле
[`RingBuffer::closed`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.closed).


### Задача 1 --- реализация циклического буфера


#### Сериализация и десериализация заголовка

Реализуйте
[метод](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.read_header)

```rust
fn RingBuffer::read_header(
    &mut self,
    position: usize,
) -> Header
```

и
[метод](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.write_header)

```rust
fn RingBuffer::write_header(
    &mut self,
    position: usize,
    header: Header,
)
```

в файле
[`ku/src/ring_buffer.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/ring_buffer.rs).
Они должен десериализовать и сериализовать заголовок `header` в циклическом буфере по позиции `position`.
Подумайте над последовательностью операций в них и необходимыми
[`Ordering`](https://doc.rust-lang.org/nightly/core/sync/atomic/enum.Ordering.html).

Также учтите, что при записи заголовка может оказаться, что в этот же заголовок конкурентно пишется значение
[`Header::Closed`](../../doc/ku/ring_buffer/enum.Header.html#variant.Closed).
Оно имеет преимущество и не должно быть потеряно, а запись другого заголовка поверх него случиться не должна.
За исключением ситуации, когда позиция заголовка ещё не использована.
То есть, когда логически буфер не дошёл до неё.
В этом случае писатель заранее записывает заголовок
[`Header::Clear`](../../doc/ku/ring_buffer/enum.Header.html#variant.Clear)
сразу после текущей записи с данными.
Это нужно, так как буфер всё же циклический и переиспользует одну и ту же память.
А значит, хотя логически в этой позиции ещё ничего не писалось, но из-за взятия позиции по модулю,
она попадает на какие-то старые данные в буфере.
Которые могли быть даже не данными заголовка, а передаваемыми пользовательскими.
И случайно могут выглядеть как произвольный заголовок,
в том числе как
[`Header::Closed`](../../doc/ku/ring_buffer/enum.Header.html#variant.Closed).

Также реализуйте
[метод](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.close)

```rust
fn RingBuffer::close(
    &mut self,
)
```

который закрывает буфер на чтение и запись.

Вам пригодятся методы:

- [`RingBuffer::state()`](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.state), --- возвращает [`AtomicU8`](https://doc.rust-lang.org/nightly/core/sync/atomic/struct.AtomicU8.html) с состоянием записи.
- [`RingBuffer::read_size()`](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.read_size), --- читает размер записи из её заголовка.
- [`RingBuffer::write_size()`](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.write_size), --- записывает размер записи в её заголовок.

В этих методах запись идентифицируется позицией своего заголовка `position` ---
это количество байт записанных до этой записи с начала работы
[`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html).
Оно только возрастает, так как содержит значение до взятия по модулю размера буфера.


#### Получение актуальных головы и хвоста

Реализуйте
[метод](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.head_tail)

```rust
fn RingBuffer::head_tail(
    &mut self,
) -> Option<(usize, usize)>
```

Он должен обновить поля
[`RingBuffer::head`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.head)
и
[`RingBuffer::tail`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.tail).
Для этого, отталкиваясь от их текущих значений, он читает заголовки последующих записей методом
[`RingBuffer::read_header()`](../../doc/ku/ring_buffer/struct.RingBuffer.html#method.read_header).
И продвигает
[`RingBuffer::head`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.head)
и
[`RingBuffer::tail`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.tail)
в соответствии с состояниями записей в прочитанных заголовках.
Если в голове находится запись в состоянии
[`Header::Closed`](../../doc/ku/ring_buffer/enum.Header.html#variant.Closed),
это означает что буфер закрыт и метод возвращает `None`.
Кроме того, при возникновении по мере чтения заголовков любых неконсистентностей или нарушений инвариантов,
буфер также считается закрытым.
Если всё хорошо, возвращается пара из новых значений `head` и `tail`.


#### Читающая транзакция

Реализуйте [метод](../../doc/ku/ring_buffer/struct.RingBufferTx.html#method.read)

```rust
fn RingBufferTx<'_, ReadTag>::read(
    &mut self,
) -> Option<&[u8]>
```

который возвращает в виде среза полезную нагрузку очередной записи из буфера.
Или `None`, если в этой читающей транзакции больше нет записей.
При этом он обновляет поля только самой транзакции
[`RingBufferTx`](../../doc/ku/ring_buffer/struct.RingBufferTx.html).

Реализуйте
[метод](../../doc/ku/ring_buffer/struct.RingBufferTx.html#method.commit)

```rust
fn RingBufferTx<'_, ReadTag>::commit(
    mut self,
)
```

который коммитит читающую транзакцию, обновляя значение
[`RingBuffer::head`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.head)
и статистики
[`RingBuffer::stats`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.stats).


#### Пишущая транзакция

Реализуйте [метод](../../doc/ku/ring_buffer/struct.RingBufferTx.html#method.write)

```rust
fn RingBufferTx<'_, WriteTag>::write(
    &mut self,
    data: &[u8],
) -> Result<()>
```

который копирует в буфер байты среза `data`, обновляя поля самой транзакции
[`RingBufferTx`](../../doc/ku/ring_buffer/struct.RingBufferTx.html)
и статистику ошибок в
[`RingBuffer::stats`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.stats).
Но не трогая поля
[`RingBuffer::head`](../../doc/ku/ring_buffer/struct.RingBufferTx.html#structfield.head)
и
[`RingBuffer::tail`](../../doc/ku/ring_buffer/struct.RingBufferTx.html#structfield.tail).
Если в буфере не остаётся места под `data`, верните ошибку
[`Error::Overflow`](../../doc/ku/ring_buffer/enum.Error.html#variant.Overflow):

```rust
pub enum Error {
    Overflow {
        capacity: usize,
        len: usize,
        exceeding_object_len: usize,
    },
}
```

С информацией

- Об остававшемся в буфере месте на момент старта транзакции. То есть, полной доступной для транзакции ёмкости, --- [`Error::Overflow::capacity`](../../doc/ku/ring_buffer/enum.Error.html#variant.Overflow.field.capacity).
- Об уже записанном ранее в рамках той же транзакции объёме --- [`Error::Overflow::len`](../../doc/ku/ring_buffer/enum.Error.html#variant.Overflow.field.len).
- О размере объекта, который не влез --- [`Error::Overflow::exceeding_object_len`](../../doc/ku/ring_buffer/enum.Error.html#variant.Overflow.field.exceeding_object_len).

Вам может пригодиться метод
[`copy_from_slice()`](https://doc.rust-lang.org/nightly/core/primitive.slice.html#method.copy_from_slice)
срезов.

Реализуйте [метод](../../doc/ku/ring_buffer/struct.RingBufferTx.html#method.commit-1)

```rust
fn RingBufferTx<'_, WriteTag>::commit(
    mut self,
)
```

который коммитит пишущую транзакцию, обновляя значение
[`RingBuffer::tail`](../../doc/ku/ring_buffer/struct.RingBufferTx.html#structfield.tail)
и статистики
[`RingBuffer::stats`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.stats).

Учтите, что читатель может прочитать внутри своей транзакции несколько записей и
закоммитить их все внутри одной своей транзакции.


#### Сброс транзакции

Реализуйте типаж
[`core::ops::Drop`](https://doc.rust-lang.org/nightly/core/ops/trait.Drop.html)
для транзакций обоих типов ---
[метод](../../doc/ku/ring_buffer/struct.RingBufferTx.html#method.drop)

```rust
fn RingBufferTx::drop(
    &mut self,
)
```

Он обновляет статистики
[`RingBuffer::stats`](../../doc/ku/ring_buffer/struct.RingBuffer.html#structfield.stats),
если в транзакции был прочитан или записан хотя бы один байт.


#### Отображение буфера в память


Реализуйте
[функцию](../../doc/kernel/process/fn.map_log.html)

```rust
fn make_pipe<T: BigAllocator>(
    allocator: &mut T,
) -> error::Result<(ReadBuffer, WriteBuffer)>
```

Она должна создать двойное отображение памяти буфера в адресное пространство,
за которое отвечает `allocator`.
То есть, она должна выделить
[`RingBuffer::REAL_SIZE`](../../doc/ku/ring_buffer/struct.RingBuffer.html#associatedconstant.REAL_SIZE)
байт в физической памяти и
[`RingBuffer::MAPPED_SIZE`](../../doc/ku/ring_buffer/struct.RingBuffer.html#associatedconstant.MAPPED_SIZE) ---
в виртуальной.
И построить отображение выделенных физических фреймов дважды подряд.
Также необходимо записать в начало буфера заголовок
[`Header::Clear`](../../doc/ku/ring_buffer/enum.Header.html#variant.Clear).

Для создания
[`ReadBuffer`](../../doc/ku/ring_buffer/type.ReadBuffer.html)
и
[`WriteBuffer`](../../doc/ku/ring_buffer/type.WriteBuffer.html)
можно использовать конструкции

```ring_buffer
Buffer::<WriteTag>(..., PhantomData);
Buffer::<ReadTag>(..., PhantomData);
```



#### Сброс буфера

За сброс буфера с сообщениями процесса пользователя в общий лог отвечает ядро.
Сам сброс осуществляет функция
[`Process::flush_log()`](../../doc/kernel/process/process/struct.Process.html#method.flush_log).
А вызываться она должна:

- Из метода [`Process::trap()`](../../doc/kernel/process/process/struct.Process.html#method.trap) при исключениях в коде пользователя. Это уже делается.
- Из диспетчера системных вызовов [`kernel::process::syscall::syscall()`](../../doc/kernel/process/syscall/fn.syscall.html). Это вам придётся добавить самостоятельно. Чтобы не поменялся логический порядок записей, относящихся к одному процессу, сделайте сброс до того как ядро залогирует какое-либо своё сообщение при обработке системного вызова.


#### Структурированное логирование в пространстве пользователя

После того как вы реализуете
[`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html),
наконец-то в пространстве пользователя заработает
структурированное логирование макросами библиотеки
[tracing](https://docs.rs/tracing/) ---
`info!()`, `debug!()` и т.д.

Устроено оно так.
В пространстве пользователя эти макросы приводят к сериализации сообщения вместе с его полями и
метаинформацией с помощью библиотеки [serde](https://docs.rs/serde/) в
формат, задаваемый библиотекой [postcard](https://docs.rs/postcard/).
Соответствующий код расположен в файле
[`ku/src/log.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/log.rs).

При этом никаких системных вызовов, а значит и переключений контекстов, не происходит до тех пор,
пока буфер не переполнится.
В случае переполнения, вызывается
[`lib::syscall::sched_yield()`](../../doc/lib/syscall/fn.sched_yield.html).
При попадании в ядро при любом системном вызове или исключении,
ядро сбрасывает в общий лог накопившиеся в
[`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html)
записи функцией
[`Process::flush_log()`](../../doc/kernel/process/process/struct.Process.html#method.flush_log).
Благодаря этому они не потеряются даже если приложение упадёт по ошибке.

После сброса переполнившегося буфера, выполняется повторная попытка записать в него не поместившееся сообщение.
Тут и пригождается откат транзакции.
Если и на этот раз сообщение не удалось записать --- видимо оно слишком большое и не помещается в буфер ---
выполняется запись небольшого префикса сообщения и
дополнительное служебное сообщение об ошибке полной записи некоторых сообщений.

> Подумайте, к каким проблемам может привести такая схема.


### Проверьте себя

Теперь должен заработать тест `5-um-1-ring-buffer` в файле
[`ku/tests/5-um-1-ring-buffer.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/5-um-1-ring-buffer.rs):

```console
$ (cd ku; cargo test --test 5-um-1-ring-buffer -- --nocapture --test-threads=1)
...
running 5 tests
test check_continuous_mapping ... 2023-11-12T19:52:21.306207Z : ThreadId(03) ring_buffer_block=[0v7F24_0DA4_A000, 0v7F24_0DA5_2000), size 32.000 KiB
ok
test concurrent ... 2023-11-12T19:52:21.808266Z : read_thread iteration=0 read_tx=69323 throughput_per_second=84.375 MiB window_bytes=44237229 read_stats=RingBufferStats { commited: 22712700, commits: 34693, dropped: 21524529, drops: 2107, errors: 0, txs: 69324 }
2023-11-12T19:52:21.808366Z : write_thread iteration=0 write_tx=3601 write_stats=RingBufferStats { commited: 22712700, commits: 2762, dropped: 15589937, drops: 5678, errors: 32775, txs: 36376 }
2023-11-12T19:52:22.308398Z :  read_thread iteration=0 read_tx=140326 throughput_per_second=81.723 MiB window_bytes=42846219 read_stats=RingBufferStats { commited: 44844551, commits: 70227, dropped: 42238897, drops: 4144, errors: 0, txs: 140327 }
2023-11-12T19:52:22.308447Z : write_thread iteration=0 write_tx=7210 write_stats=RingBufferStats { commited: 44844551, commits: 5450, dropped: 32238329, drops: 11639, errors: 65594, txs: 72804 }
2023-11-12T19:52:23.192529Z :  read_thread iteration=1 read_tx=68767 throughput_per_second=84.235 MiB window_bytes=44172197 read_stats=RingBufferStats { commited: 22660430, commits: 34439, dropped: 21511767, drops: 2092, errors: 0, txs: 68768 }
2023-11-12T19:52:23.192579Z : write_thread iteration=1 write_tx=3639 write_stats=RingBufferStats { commited: 22668544, commits: 2764, dropped: 18504829, drops: 6300, errors: 37113, txs: 40752 }
2023-11-12T19:52:23.692783Z : write_thread iteration=1 write_tx=7340 write_stats=RingBufferStats { commited: 45513134, commits: 5572, dropped: 36808688, drops: 12732, errors: 73855, txs: 81195 }
2023-11-12T19:52:23.692819Z :  read_thread iteration=1 read_tx=137101 throughput_per_second=85.916 MiB window_bytes=45058725 read_stats=RingBufferStats { commited: 45513134, commits: 68654, dropped: 43717788, drops: 4216, errors: 0, txs: 137102 }
2023-11-12T19:52:24.551556Z :  read_thread iteration=2 read_tx=69153 throughput_per_second=82.674 MiB window_bytes=43345128 read_stats=RingBufferStats { commited: 22209155, commits: 34752, dropped: 21135973, drops: 2057, errors: 0, txs: 69154 }
2023-11-12T19:52:24.551618Z : write_thread iteration=2 write_tx=3696 write_stats=RingBufferStats { commited: 22209155, commits: 2739, dropped: 19173939, drops: 6499, errors: 35893, txs: 39589 }
2023-11-12T19:52:25.051871Z :  read_thread iteration=2 read_tx=136700 throughput_per_second=83.770 MiB window_bytes=43930049 read_stats=RingBufferStats { commited: 44860375, commits: 68587, dropped: 42414802, drops: 4159, errors: 0, txs: 136701 }
2023-11-12T19:52:25.052015Z : write_thread iteration=2 write_tx=7416 write_stats=RingBufferStats { commited: 44875078, commits: 5540, dropped: 37104985, drops: 12867, errors: 72384, txs: 79800 }
2023-11-12T19:52:25.903372Z : write_thread iteration=3 write_tx=3588 write_stats=RingBufferStats { commited: 21848127, commits: 2666, dropped: 19460906, drops: 6711, errors: 38016, txs: 41604 }
2023-11-12T19:52:25.903459Z :  read_thread iteration=3 read_tx=66039 throughput_per_second=79.772 MiB window_bytes=41837923 read_stats=RingBufferStats { commited: 21848127, commits: 33096, dropped: 19989796, drops: 1954, errors: 0, txs: 66040 }
2023-11-12T19:52:26.403615Z :  read_thread iteration=3 read_tx=132322 throughput_per_second=82.199 MiB window_bytes=43101965 read_stats=RingBufferStats { commited: 43788016, commits: 66178, dropped: 41151872, drops: 3949, errors: 0, txs: 132323 }
2023-11-12T19:52:26.403678Z : write_thread iteration=3 write_tx=7118 write_stats=RingBufferStats { commited: 43798492, commits: 5294, dropped: 38380789, drops: 13220, errors: 77381, txs: 84499 }
2023-11-12T19:52:27.316664Z :  read_thread iteration=4 read_tx=68201 throughput_per_second=83.010 MiB window_bytes=43528259 read_stats=RingBufferStats { commited: 22559379, commits: 34317, dropped: 20968880, drops: 2102, errors: 0, txs: 68202 }
2023-11-12T19:52:27.316812Z : write_thread iteration=4 write_tx=3683 write_stats=RingBufferStats { commited: 22571422, commits: 2767, dropped: 18518952, drops: 6574, errors: 36349, txs: 40032 }
2023-11-12T19:52:27.816984Z :  read_thread iteration=4 read_tx=135473 throughput_per_second=83.836 MiB window_bytes=43969359 read_stats=RingBufferStats { commited: 45402091, commits: 67891, dropped: 42095527, drops: 4154, errors: 0, txs: 135474 }
2023-11-12T19:52:27.817004Z : write_thread iteration=4 write_tx=7385 write_stats=RingBufferStats { commited: 45406911, commits: 5548, dropped: 36254608, drops: 12830, errors: 72904, txs: 80289 }
2023-11-12T19:52:28.667193Z : write_thread iteration=5 write_tx=3698 write_stats=RingBufferStats { commited: 22874812, commits: 2761, dropped: 18095511, drops: 6085, errors: 36770, txs: 40468 }
2023-11-12T19:52:28.667193Z :  read_thread iteration=5 read_tx=67879 throughput_per_second=85.057 MiB window_bytes=44594564 read_stats=RingBufferStats { commited: 22874812, commits: 33926, dropped: 21719752, drops: 2093, errors: 0, txs: 67880 }
2023-11-12T19:52:29.167421Z :  read_thread iteration=5 read_tx=136354 throughput_per_second=84.950 MiB window_bytes=44538395 read_stats=RingBufferStats { commited: 45729519, commits: 68176, dropped: 43403440, drops: 4173, errors: 0, txs: 136355 }
2023-11-12T19:52:29.167594Z : write_thread iteration=5 write_tx=7375 write_stats=RingBufferStats { commited: 45729519, commits: 5543, dropped: 36346938, drops: 12472, errors: 74106, txs: 81481 }
2023-11-12T19:52:30.021125Z : write_thread iteration=6 write_tx=3727 write_stats=RingBufferStats { commited: 22404773, commits: 2806, dropped: 19147120, drops: 6733, errors: 36144, txs: 39871 }
2023-11-12T19:52:30.021120Z :  read_thread iteration=6 read_tx=68547 throughput_per_second=83.884 MiB window_bytes=43980540 read_stats=RingBufferStats { commited: 22404773, commits: 33900, dropped: 21575767, drops: 2106, errors: 0, txs: 68548 }
2023-11-12T19:52:30.521303Z : write_thread iteration=6 write_tx=7400 write_stats=RingBufferStats { commited: 45085806, commits: 5555, dropped: 38308302, drops: 13116, errors: 72358, txs: 79758 }
2023-11-12T19:52:30.521303Z :  read_thread iteration=6 read_tx=136891 throughput_per_second=82.315 MiB window_bytes=43156808 read_stats=RingBufferStats { commited: 45078305, commits: 67983, dropped: 42059043, drops: 4083, errors: 0, txs: 136892 }
2023-11-12T19:52:31.382838Z : write_thread iteration=7 write_tx=3606 write_stats=RingBufferStats { commited: 22572729, commits: 2730, dropped: 17497302, drops: 6043, errors: 35576, txs: 39182 }
2023-11-12T19:52:31.382973Z :  read_thread iteration=7 read_tx=68347 throughput_per_second=82.725 MiB window_bytes=43387343 read_stats=RingBufferStats { commited: 22572729, commits: 34383, dropped: 20814614, drops: 2036, errors: 0, txs: 68348 }
2023-11-12T19:52:31.883077Z : write_thread iteration=7 write_tx=7311 write_stats=RingBufferStats { commited: 45308170, commits: 5516, dropped: 35484786, drops: 12216, errors: 70716, txs: 78027 }
2023-11-12T19:52:31.883070Z :  read_thread iteration=7 read_tx=137032 throughput_per_second=82.480 MiB window_bytes=43243553 read_stats=RingBufferStats { commited: 45308170, commits: 68469, dropped: 41322726, drops: 4052, errors: 0, txs: 137033 }
2023-11-12T19:52:32.754459Z :  read_thread iteration=8 read_tx=69134 throughput_per_second=80.648 MiB window_bytes=42288345 read_stats=RingBufferStats { commited: 22225655, commits: 34374, dropped: 20062690, drops: 1936, errors: 0, txs: 69135 }
2023-11-12T19:52:32.754540Z : write_thread iteration=8 write_tx=3627 write_stats=RingBufferStats { commited: 22236394, commits: 2667, dropped: 19337059, drops: 6249, errors: 36119, txs: 39746 }
2023-11-12T19:52:33.254653Z :  read_thread iteration=8 read_tx=136968 throughput_per_second=82.453 MiB window_bytes=43234462 read_stats=RingBufferStats { commited: 44900680, commits: 68099, dropped: 40622127, drops: 3987, errors: 0, txs: 136969 }
2023-11-12T19:52:33.254767Z : write_thread iteration=8 write_tx=7311 write_stats=RingBufferStats { commited: 44915576, commits: 5431, dropped: 38388675, drops: 12869, errors: 71364, txs: 78675 }
2023-11-12T19:52:34.116822Z :  read_thread iteration=9 read_tx=68621 throughput_per_second=81.167 MiB window_bytes=42555082 read_stats=RingBufferStats { commited: 22477495, commits: 34435, dropped: 20077587, drops: 2029, errors: 0, txs: 68622 }
2023-11-12T19:52:34.116916Z : write_thread iteration=9 write_tx=3685 write_stats=RingBufferStats { commited: 22493863, commits: 2766, dropped: 18678299, drops: 6126, errors: 36516, txs: 40201 }
2023-11-12T19:52:34.617104Z :  read_thread iteration=9 read_tx=137632 throughput_per_second=82.408 MiB window_bytes=43217791 read_stats=RingBufferStats { commited: 44908732, commits: 68866, dropped: 40864141, drops: 4082, errors: 0, txs: 137633 }
2023-11-12T19:52:34.617219Z : write_thread iteration=9 write_tx=7264 write_stats=RingBufferStats { commited: 44914850, commits: 5438, dropped: 37398833, drops: 12528, errors: 73495, txs: 80759 }
2023-11-12T19:52:35.485207Z :  read_thread iteration=10 read_tx=68252 throughput_per_second=82.804 MiB window_bytes=43413057 read_stats=RingBufferStats { commited: 22612958, commits: 34067, dropped: 20800099, drops: 2032, errors: 0, txs: 68253 }
2023-11-12T19:52:35.485248Z : write_thread iteration=10 write_tx=3687 write_stats=RingBufferStats { commited: 22612958, commits: 2752, dropped: 18398249, drops: 6225, errors: 36618, txs: 40305 }
2023-11-12T19:52:35.985357Z :  read_thread iteration=10 read_tx=137085 throughput_per_second=82.757 MiB window_bytes=43388337 read_stats=RingBufferStats { commited: 45110463, commits: 68442, dropped: 41690931, drops: 4063, errors: 0, txs: 137086 }
2023-11-12T19:52:35.985451Z : write_thread iteration=10 write_tx=7324 write_stats=RingBufferStats { commited: 45125897, commits: 5456, dropped: 37282255, drops: 12432, errors: 73285, txs: 80609 }
2023-11-12T19:52:36.847501Z :  read_thread iteration=11 read_tx=69581 throughput_per_second=84.385 MiB window_bytes=44242447 read_stats=RingBufferStats { commited: 22587413, commits: 34742, dropped: 21655034, drops: 2124, errors: 0, txs: 69582 }
2023-11-12T19:52:36.847526Z : write_thread iteration=11 write_tx=3702 write_stats=RingBufferStats { commited: 22587413, commits: 2768, dropped: 18006864, drops: 6147, errors: 33460, txs: 37162 }
2023-11-12T19:52:37.347666Z :  read_thread iteration=11 read_tx=139317 throughput_per_second=78.768 MiB window_bytes=41297346 read_stats=RingBufferStats { commited: 44641083, commits: 69743, dropped: 40898710, drops: 4027, errors: 0, txs: 139318 }
2023-11-12T19:52:37.347762Z : write_thread iteration=11 write_tx=7309 write_stats=RingBufferStats { commited: 44651094, commits: 5439, dropped: 35093918, drops: 12370, errors: 68210, txs: 75519 }
2023-11-12T19:52:38.216991Z :  read_thread iteration=12 read_tx=64707 throughput_per_second=78.220 MiB window_bytes=41009899 read_stats=RingBufferStats { commited: 21504116, commits: 32311, dropped: 19505783, drops: 1937, errors: 0, txs: 64708 }
2023-11-12T19:52:38.217104Z : write_thread iteration=12 write_tx=3462 write_stats=RingBufferStats { commited: 21515230, commits: 2626, dropped: 18622637, drops: 6655, errors: 41772, txs: 45234 }
2023-11-12T19:52:38.717129Z :  read_thread iteration=12 read_tx=127971 throughput_per_second=80.635 MiB window_bytes=42276022 read_stats=RingBufferStats { commited: 43514035, commits: 63994, dropped: 39771886, drops: 3918, errors: 0, txs: 127972 }
2023-11-12T19:52:38.717201Z : write_thread iteration=12 write_tx=6951 write_stats=RingBufferStats { commited: 43514035, commits: 5282, dropped: 37288428, drops: 13391, errors: 82031, txs: 88982 }
2023-11-12T19:52:39.660248Z : write_thread iteration=13 write_tx=3574 write_stats=RingBufferStats { commited: 22470710, commits: 2675, dropped: 18497081, drops: 6304, errors: 36759, txs: 40333 }
2023-11-12T19:52:39.660330Z :  read_thread iteration=13 read_tx=68423 throughput_per_second=83.753 MiB window_bytes=43922211 read_stats=RingBufferStats { commited: 22470710, commits: 34357, dropped: 21451501, drops: 2086, errors: 0, txs: 68424 }
2023-11-12T19:52:40.160459Z :  read_thread iteration=13 read_tx=137104 throughput_per_second=85.794 MiB window_bytes=44984302 read_stats=RingBufferStats { commited: 45298873, commits: 68659, dropped: 43607640, drops: 4200, errors: 0, txs: 137105 }
2023-11-12T19:52:40.160518Z : write_thread iteration=13 write_tx=7108 write_stats=RingBufferStats { commited: 45307132, commits: 5362, dropped: 35940888, drops: 12278, errors: 74410, txs: 81518 }
2023-11-12T19:52:41.063212Z : write_thread iteration=14 write_tx=3640 write_stats=RingBufferStats { commited: 22263424, commits: 2746, dropped: 19372363, drops: 6568, errors: 35293, txs: 38933 }
2023-11-12T19:52:41.063219Z :  read_thread iteration=14 read_tx=68244 throughput_per_second=81.461 MiB window_bytes=42713269 read_stats=RingBufferStats { commited: 22263424, commits: 34078, dropped: 20449845, drops: 2008, errors: 0, txs: 68245 }
2023-11-12T19:52:41.563432Z :  read_thread iteration=14 read_tx=136678 throughput_per_second=85.650 MiB window_bytes=44908555 read_stats=RingBufferStats { commited: 45231269, commits: 68545, dropped: 42390555, drops: 4124, errors: 0, txs: 136679 }
2023-11-12T19:52:41.563487Z : write_thread iteration=14 write_tx=7322 write_stats=RingBufferStats { commited: 45231269, commits: 5512, dropped: 37558159, drops: 13013, errors: 72496, txs: 79818 }
2023-11-12T19:52:42.424823Z : write_thread iteration=15 write_tx=3651 write_stats=RingBufferStats { commited: 22778899, commits: 2763, dropped: 18142657, drops: 6222, errors: 35883, txs: 39534 }
2023-11-12T19:52:42.424823Z :  read_thread iteration=15 read_tx=68339 throughput_per_second=83.384 MiB window_bytes=43725941 read_stats=RingBufferStats { commited: 22778899, commits: 34394, dropped: 20947042, drops: 2032, errors: 0, txs: 68340 }
2023-11-12T19:52:42.924998Z :  read_thread iteration=15 read_tx=137238 throughput_per_second=82.913 MiB window_bytes=43470409 read_stats=RingBufferStats { commited: 45511299, commits: 68431, dropped: 41685051, drops: 4079, errors: 0, txs: 137239 }
2023-11-12T19:52:42.925101Z : write_thread iteration=15 write_tx=7337 write_stats=RingBufferStats { commited: 45511299, commits: 5555, dropped: 36062737, drops: 12508, errors: 72471, txs: 79808 }
2023-11-12T19:52:43.787817Z : write_thread iteration=16 write_tx=3386 write_stats=RingBufferStats { commited: 21102410, commits: 2518, dropped: 19332992, drops: 6863, errors: 42512, txs: 45898 }
2023-11-12T19:52:43.787806Z :  read_thread iteration=16 read_tx=62659 throughput_per_second=78.046 MiB window_bytes=40936188 read_stats=RingBufferStats { commited: 21102410, commits: 31171, dropped: 19833778, drops: 1888, errors: 0, txs: 62660 }
2023-11-12T19:52:44.287940Z : write_thread iteration=16 write_tx=6776 write_stats=RingBufferStats { commited: 41979025, commits: 5038, dropped: 39996296, drops: 14137, errors: 83781, txs: 90557 }
2023-11-12T19:52:44.287997Z :  read_thread iteration=16 read_tx=125639 throughput_per_second=79.435 MiB window_bytes=41646818 read_stats=RingBufferStats { commited: 41979025, commits: 62594, dropped: 40603981, drops: 3904, errors: 0, txs: 125640 }
2023-11-12T19:52:45.258417Z : write_thread iteration=17 write_tx=3687 write_stats=RingBufferStats { commited: 22367971, commits: 2761, dropped: 19158285, drops: 6683, errors: 36786, txs: 40473 }
2023-11-12T19:52:45.258417Z :  read_thread iteration=17 read_tx=67326 throughput_per_second=82.976 MiB window_bytes=43503269 read_stats=RingBufferStats { commited: 22367971, commits: 33452, dropped: 21135298, drops: 2108, errors: 0, txs: 67327 }
2023-11-12T19:52:45.758593Z : write_thread iteration=17 write_tx=7384 write_stats=RingBufferStats { commited: 44717857, commits: 5517, dropped: 38115595, drops: 13137, errors: 73404, txs: 80788 }
2023-11-12T19:52:45.758601Z :  read_thread iteration=17 read_tx=134661 throughput_per_second=80.273 MiB window_bytes=42086189 read_stats=RingBufferStats { commited: 44717857, commits: 67199, dropped: 40871601, drops: 4075, errors: 0, txs: 134662 }
2023-11-12T19:52:46.626783Z : write_thread iteration=18 write_tx=3662 write_stats=RingBufferStats { commited: 22395805, commits: 2737, dropped: 19335345, drops: 6429, errors: 36493, txs: 40155 }
2023-11-12T19:52:46.626877Z :  read_thread iteration=18 read_tx=67454 throughput_per_second=81.111 MiB window_bytes=42538377 read_stats=RingBufferStats { commited: 22395805, commits: 33871, dropped: 20142572, drops: 1939, errors: 0, txs: 67455 }
2023-11-12T19:52:47.127039Z :  read_thread iteration=18 read_tx=135071 throughput_per_second=83.772 MiB window_bytes=43927242 read_stats=RingBufferStats { commited: 45092959, commits: 67693, dropped: 41372660, drops: 3970, errors: 0, txs: 135072 }
2023-11-12T19:52:47.127142Z : write_thread iteration=18 write_tx=7338 write_stats=RingBufferStats { commited: 45105630, commits: 5499, dropped: 37783861, drops: 12934, errors: 74233, txs: 81571 }
2023-11-12T19:52:47.987469Z : write_thread iteration=19 write_tx=3640 write_stats=RingBufferStats { commited: 22286847, commits: 2696, dropped: 18460777, drops: 6324, errors: 37111, txs: 40751 }
2023-11-12T19:52:47.987483Z :  read_thread iteration=19 read_tx=68926 throughput_per_second=84.734 MiB window_bytes=44428050 read_stats=RingBufferStats { commited: 22286847, commits: 34308, dropped: 22141203, drops: 2121, errors: 0, txs: 68927 }
2023-11-12T19:52:48.487699Z : write_thread iteration=19 write_tx=7350 write_stats=RingBufferStats { commited: 44693096, commits: 5455, dropped: 38045762, drops: 13022, errors: 72831, txs: 80181 }
2023-11-12T19:52:48.487795Z :  read_thread iteration=19 read_tx=137412 throughput_per_second=82.442 MiB window_bytes=43235171 read_stats=RingBufferStats { commited: 44693096, commits: 68488, dropped: 42970125, drops: 4129, errors: 0, txs: 137413 }
ok
test reader_close ... 2023-11-12T19:52:48.845387Z : ThreadId(47) the RingBuffer is closed concurrently or corrupted position=0 old=1 header=Written { size: 3 }
2023-11-12T19:52:48.845799Z X ThreadId(47) state=165 position=0 RingBuffer is corrupted by the other size, treating it as closed
2023-11-12T19:52:48.845850Z X ThreadId(47) state=165 position=6 RingBuffer is corrupted by the other size, treating it as closed
2023-11-12T19:52:48.845907Z X ThreadId(47) state=165 position=0 RingBuffer is corrupted by the other size, treating it as closed
2023-11-12T19:52:48.846303Z X ThreadId(47) invalid state transition, closing the RingBuffer position=0 old=165 header=Written { size: 3 }
ok
test sequential ... 2023-11-12T19:52:48.847151Z : ThreadId(49) iteration=0 read_stats=RingBufferStats { commited: 0, commits: 0, dropped: 0, drops: 0, errors: 0, txs: 0 } write_stats=RingBufferStats { commited: 0, commits: 0, dropped: 0, drops: 0, errors: 0, txs: 0 }
2023-11-12T19:52:48.992553Z : ThreadId(49) iteration=1000 read_stats=RingBufferStats { commited: 4777049, commits: 450, dropped: 4565906, drops: 392, errors: 0, txs: 875 } write_stats=RingBufferStats { commited: 4785218, commits: 570, dropped: 2789082, drops: 376, errors: 619, txs: 1363 }
2023-11-12T19:52:49.137571Z : ThreadId(49) iteration=2000 read_stats=RingBufferStats { commited: 9660027, commits: 907, dropped: 9490403, drops: 807, errors: 0, txs: 1789 } write_stats=RingBufferStats { commited: 9660027, commits: 1143, dropped: 5401742, drops: 765, errors: 1283, txs: 2777 }
2023-11-12T19:52:49.279328Z : ThreadId(49) iteration=3000 read_stats=RingBufferStats { commited: 14364668, commits: 1352, dropped: 13764976, drops: 1184, errors: 0, txs: 2658 } write_stats=RingBufferStats { commited: 14375257, commits: 1714, dropped: 8201466, drops: 1195, errors: 1892, txs: 4126 }
2023-11-12T19:52:49.422557Z : ThreadId(49) iteration=4000 read_stats=RingBufferStats { commited: 19098946, commits: 1788, dropped: 18589163, drops: 1612, errors: 0, txs: 3554 } write_stats=RingBufferStats { commited: 19098946, commits: 2307, dropped: 11035985, drops: 1630, errors: 2552, txs: 5550 }
2023-11-12T19:52:49.563438Z : ThreadId(49) iteration=5000 read_stats=RingBufferStats { commited: 23601436, commits: 2209, dropped: 22624900, drops: 1972, errors: 0, txs: 4389 } write_stats=RingBufferStats { commited: 23612241, commits: 2868, dropped: 13984171, drops: 2060, errors: 3137, txs: 6885 }
2023-11-12T19:52:49.705967Z : ThreadId(49) iteration=6000 read_stats=RingBufferStats { commited: 28057233, commits: 2624, dropped: 28004170, drops: 2420, errors: 0, txs: 5299 } write_stats=RingBufferStats { commited: 28073548, commits: 3423, dropped: 16920182, drops: 2508, errors: 3817, txs: 8335 }
2023-11-12T19:52:49.848216Z : ThreadId(49) iteration=7000 read_stats=RingBufferStats { commited: 32660467, commits: 3074, dropped: 32279073, drops: 2792, errors: 0, txs: 6156 } write_stats=RingBufferStats { commited: 32661575, commits: 3994, dropped: 19779259, drops: 2926, errors: 4426, txs: 9696 }
2023-11-12T19:52:49.995969Z : ThreadId(49) iteration=8000 read_stats=RingBufferStats { commited: 37586632, commits: 3531, dropped: 37213399, drops: 3192, errors: 0, txs: 7056 } write_stats=RingBufferStats { commited: 37593446, commits: 4552, dropped: 22194951, drops: 3320, errors: 5066, txs: 11076 }
2023-11-12T19:52:50.136140Z : ThreadId(49) iteration=9000 read_stats=RingBufferStats { commited: 42103182, commits: 3952, dropped: 41570432, drops: 3565, errors: 0, txs: 7892 } write_stats=RingBufferStats { commited: 42113269, commits: 5109, dropped: 24716620, drops: 3697, errors: 5642, txs: 12392 }
ok
test writer_close ... 2023-11-12T19:52:50.269792Z X ThreadId(51) state=165 position=0 RingBuffer is corrupted by the other size, treating it as closed
2023-11-12T19:52:50.269961Z X ThreadId(51) state=165 position=0 RingBuffer is corrupted by the other size, treating it as closed
2023-11-12T19:52:50.270088Z X ThreadId(51) state=165 position=0 RingBuffer is corrupted by the other size, treating it as closed
ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.97s
```

Если вам придётся его отлаживать, стоит поменять в нём константу
```rust
const QUALITY: Quality = Quality::Paranoid;
```
на
```rust
const QUALITY: Quality = Quality::Debuggable;
```
а вызовы `trace!()` на `debug!()`:

```console
...
test concurrent ... 2023-11-12T20:08:42.276650Z : read_thread read commit
2023-11-12T20:08:42.276724Z : read_thread read commit
2023-11-12T20:08:42.276780Z : read_thread read commit
2023-11-12T20:08:42.276827Z :  read_thread read commit
2023-11-12T20:08:42.276819Z : write_thread write operation='o' x 9697, commit chunk_count=20
2023-11-12T20:08:42.276866Z :  read_thread read commit
2023-11-12T20:08:42.277053Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 5921, exceeding_object_len: 4358 })
2023-11-12T20:08:42.277117Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 0, exceeding_object_len: 10228 })
2023-11-12T20:08:42.277166Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 0, exceeding_object_len: 8301 })
2023-11-12T20:08:42.276924Z :  read_thread read_tx data='o' x 9697, block_count = 1, total_len = 9697 len=9697
2023-11-12T20:08:42.277233Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 5292, exceeding_object_len: 3624 })
2023-11-12T20:08:42.277303Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 3334, exceeding_object_len: 4674 })
2023-11-12T20:08:42.277351Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 0, exceeding_object_len: 6841 })
2023-11-12T20:08:42.277412Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 1113, exceeding_object_len: 9777 })
2023-11-12T20:08:42.277475Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 5083, exceeding_object_len: 6168 })
2023-11-12T20:08:42.277522Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 0, exceeding_object_len: 9443 })
2023-11-12T20:08:42.277272Z :  read_thread read_tx data='o' x 9697, block_count = 1, total_len = 9697 len=9697
2023-11-12T20:08:42.277591Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 5397, exceeding_object_len: 3839 })
2023-11-12T20:08:42.277672Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 6290, exceeding_object_len: 5389 })
2023-11-12T20:08:42.277737Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 4417, exceeding_object_len: 4514 })
2023-11-12T20:08:42.277786Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 0, exceeding_object_len: 9853 })
2023-11-12T20:08:42.277852Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 5107, exceeding_object_len: 2401 })
2023-11-12T20:08:42.277921Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 5246, exceeding_object_len: 2010 })
2023-11-12T20:08:42.277678Z :  read_thread read data='o' x 9697, block_count = 1, total_len = 9697 operation='o' x 9697, commit
2023-11-12T20:08:42.277970Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 0, exceeding_object_len: 8507 })
2023-11-12T20:08:42.278141Z :  read_thread read commit
2023-11-12T20:08:42.278135Z : write_thread operation='u' x 12074, commit write_error=RingBuffer(Overflow { capacity: 6680, len: 5329, exceeding_object_len: 2849 })
2023-11-12T20:08:42.278188Z :  read_thread read commit
2023-11-12T20:08:42.278242Z :  read_thread read commit
2023-11-12T20:08:42.278255Z : write_thread write operation='u' x 12074, commit chunk_count=11
2023-11-12T20:08:42.278282Z :  read_thread read commit
2023-11-12T20:08:42.278447Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 3083, exceeding_object_len: 8101 })
2023-11-12T20:08:42.278497Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 0, exceeding_object_len: 13983 })
2023-11-12T20:08:42.278557Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 4056, exceeding_object_len: 626 })
2023-11-12T20:08:42.278616Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 3042, exceeding_object_len: 10973 })
2023-11-12T20:08:42.278325Z :  read_thread read_tx data='u' x 12074, block_count = 1, total_len = 12074 len=12074
2023-11-12T20:08:42.278662Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 0, exceeding_object_len: 6234 })
2023-11-12T20:08:42.278709Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 0, exceeding_object_len: 15406 })
2023-11-12T20:08:42.278770Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 3711, exceeding_object_len: 3261 })
2023-11-12T20:08:42.278829Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 2805, exceeding_object_len: 5407 })
2023-11-12T20:08:42.278876Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 0, exceeding_object_len: 13186 })
2023-11-12T20:08:42.278921Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 0, exceeding_object_len: 14876 })
2023-11-12T20:08:42.278967Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 0, exceeding_object_len: 5486 })
2023-11-12T20:08:42.279012Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 0, exceeding_object_len: 9694 })
2023-11-12T20:08:42.279061Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 0, exceeding_object_len: 7083 })
2023-11-12T20:08:42.278780Z :  read_thread read data='u' x 12074, block_count = 1, total_len = 12074 operation='u' x 12074, commit
2023-11-12T20:08:42.279108Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 0, exceeding_object_len: 4811 })
2023-11-12T20:08:42.279164Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 2449, exceeding_object_len: 7741 })
2023-11-12T20:08:42.279186Z :  read_thread read commit
2023-11-12T20:08:42.279223Z : write_thread operation='E' x 15574, commit write_error=RingBuffer(Overflow { capacity: 4303, len: 3005, exceeding_object_len: 6764 })
2023-11-12T20:08:42.279229Z :  read_thread read commit
2023-11-12T20:08:42.279292Z :  read_thread read commit
2023-11-12T20:08:42.279329Z : write_thread write operation='E' x 15574, commit chunk_count=8
2023-11-12T20:08:42.279336Z :  read_thread read commit
2023-11-12T20:08:42.279512Z : write_thread operation='a' x 16369, commit write_error=RingBuffer(Overflow { capacity: 803, len: 0, exceeding_object_len: 866 })
2023-11-12T20:08:42.279560Z : write_thread operation='a' x 16369, commit write_error=RingBuffer(Overflow { capacity: 803, len: 0, exceeding_object_len: 7641 })
2023-11-12T20:08:42.279606Z : write_thread operation='a' x 16369, commit write_error=RingBuffer(Overflow { capacity: 803, len: 0, exceeding_object_len: 7534 })
2023-11-12T20:08:42.279650Z : write_thread operation='a' x 16369, commit write_error=RingBuffer(Overflow { capacity: 803, len: 0, exceeding_object_len: 14725 })
2023-11-12T20:08:42.279696Z : write_thread operation='a' x 16369, commit write_error=RingBuffer(Overflow { capacity: 803, len: 0, exceeding_object_len: 2136 })
2023-11-12T20:08:42.279742Z : write_thread operation='a' x 16369, commit write_error=RingBuffer(Overflow { capacity: 803, len: 0, exceeding_object_len: 1663 })
2023-11-12T20:08:42.279789Z : write_thread operation='a' x 16369, commit write_error=RingBuffer(Overflow { capacity: 803, len: 0, exceeding_object_len: 14790 })
2023-11-12T20:08:42.279402Z :  read_thread read_tx data='E' x 15574, block_count = 1, total_len = 15574 len=15574
...
```


> ### Классическая ограниченная Multi-Producer Multi-Consumer очередь
>
> Наш циклический буфер похож на классическую ограниченную SPSC очередь на циклическом буфере.
> [Хорошая иллюстрация работы ограниченных SPSC и MPMC очереди](https://doc.dpdk.org/guides-19.05/prog_guide/ring_lib.html).
>
> Сходства:
>
> - Циклический буфер.
> - Атомарные операции.
> - Вставка и извлечение слайса аналогичны bulk enqueu и bulk deque.
>
> Различия:
>
> - Двойное отображение в память.
> - Использование [`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html) одновременно и как очереди и как буфера памяти. Вместо указателей на отдельно выделенную память, лежащих в SPSC.
> - Поддержка транзакций и их отката.
>
> Обратите внимание, что очередь по определению сериализует писателя и читателя.
> Запись может быть прочитана только после того как она записана.
> Но в явном виде блокировок нет.
> Если в случае пустой очереди читатель сразу получает ошибку, а не блокируется,
> и аналогично писатель в случае полной очереди, то SPSC является lock free.
> Но в MPMC есть ещё и сериализация писателей между собой и сериализация читателей между собой.
> А именно, писатель захватывает слот и пока он не освободит его, писатели, захватившие последующие слоты ждут.
> То есть, если писатель, захватит слот и зависнет, то зависнут все писатели.
> Аналогично и для читателей.
> Это означает, что при таком зависании глобальный прогресс остановится, а MPMC не является lock free.
>
> Немного другие способы организации ограниченных очередей на кольцевом буфере:
>
> - [A look at some bounded queues](http://cbloomrants.blogspot.com/2011/07/07-29-11-look-at-some-bounded-queues_29.html) (SPSC).
> - [A look at some bounded queues - part 2](http://cbloomrants.blogspot.com/2011/07/07-30-11-look-at-some-bounded-queues.html) (MPMC).
>
> Тут также есть соображения о помещении атомиков с состоянием ячеек в разные кеш-линии.
> В [`RingBuffer`](../../doc/ku/ring_buffer/struct.RingBuffer.html) вы тоже можете это реализовать.
>
> Ещё один полезный цикл статей про SPSC, SPMC и MPMC lock free очереди:
>
> - [A Fast Lock-Free Queue for C++](https://moodycamel.com/blog/2013/a-fast-lock-free-queue-for-c++.htm)
> - [A Fast General Purpose Lock-Free Queue for C++](https://moodycamel.com/blog/2014/a-fast-general-purpose-lock-free-queue-for-c++.htm)
> - [Detailed Design of a Lock-Free Queue](https://moodycamel.com/blog/2014/detailed-design-of-a-lock-free-queue.htm)
>
> Классический алгоритм неограниченной MPMC на односвязном списке:
>
> - [Simple, Fast, and Practical Non-Blocking and Blocking Concurrent Queue Algorithms, Maged M. Michael and Michael L. Scott](https://cs.rochester.edu/u/scott/papers/1996_PODC_queues.pdf)
>
> Используется и гибридный подход, объединящий в односвязный список массивы или циклические буферы. Например:
>
> - [Jiffy: A Fast, Memory Efficient, Wait-Free Multi-Producers Single-Consumer Queue](https://arxiv.org/pdf/2010.14189.pdf)
> - [Fast Concurrent Queues for x86 Processors](https://www.cs.tau.ac.il/~mad/publications/ppopp2013-x86queues.pdf)


### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/ring_buffer.rs | 234 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++--------
 1 file changed, 214 insertions(+), 20 deletions(-)
```
