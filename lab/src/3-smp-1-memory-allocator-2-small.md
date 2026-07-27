### Аллокатор маленьких блоков памяти

Эта задача не обязательная, без её выполнения дальнейший код должен работать.
Её цель --- научиться тратить меньше памяти при аллокациях маленького размера.

Структура глобального аллокатора будет напоминать описанную Андреем Аксандреску
иерархическую схему:

- [CppCon 2015: Andrei Alexandrescu "std::allocator..."](https://www.youtube.com/watch?v=LIb3L4vKZ7U)

Аллокатор верхнего уровня называется
[`ku::allocator::dispatcher::Dispatcher`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html).
Сам он память не выделяет, а только диспетчеризует обращение к подходящему аллокатору
нижнего уровня.

В качестве `fallback` мы будем использовать аллокатор постраничных блоков, который реализует типаж
[`core::alloc::Allocator`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html).
Для понимания кто владеет блоком, используем размер этого блока, ---
Rust предоставляет
[`core::alloc::Layout`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html)
с размером
[`Layout::size()`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html#method.size)
при деаллокациях.

В качестве аллокатора нижнего уровня используем
[`ku::allocator::fixed_size::FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html).
Он выделяет блоки одинакового размера.
Которые хранит в структуре данных, реализующей интерфейс
[Last In, First Out](https://en.wikipedia.org/wiki/Stack_(abstract_data_type))
на основе односвязного списка записей
[`ku::allocator::fixed_size::Lifo`](../../doc/ku/allocator/fixed_size/struct.Lifo.html).
Сам
[`FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html)
не хранит размер блоков, которыми управляет.
Этот размер вычисляется на лету диспетчером
[`Dispatcher`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html)
по индексу в массиве
[`Dispatcher::fixed_size`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fixed_size)
и передаётся в методы
[`FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html)
явно.
Все размеры кратны размеру и выравниванию структуры
[`ku::allocator::fixed_size::Lifo`](../../doc/ku/allocator/fixed_size/struct.Lifo.html) ---
константе
[`ku::allocator::fixed_size::MIN_SIZE`](../../doc/ku/allocator/fixed_size/constant.MIN_SIZE.html).
Поэтому при выделении памяти мы будем округлять запрошенный размер
[`Layout::size()`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html#method.size)
и запрошенное выравнивание
[`Layout::align()`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html#method.align)
до кратного этой константе.
Или же до размера страницы
[`Page::SIZE`](../../doc/ku/memory/frage/struct.Frage.html#associatedconstant.SIZE),
при выделении напрямую из
[`Dispatcher::fallback`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fallback).
Выравнивание большего чем страница размера поддерживать не будем.


### Задача 2 --- аллокатор маленьких блоков памяти

#### Доработайте диспетчер аллокаторов [`Dispatcher`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html)

Диспетчер аллокаторов
[`Dispatcher`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html)
устанавливается в качестве глобального аллокатора
[`kernel::allocator::GLOBAL_ALLOCATOR`](../../doc/kernel/allocator/static.GLOBAL_ALLOCATOR.html)
и должен реализовывать соответствующий типаж
[`core::alloc::GlobalAlloc`](https://doc.rust-lang.org/nightly/core/alloc/trait.GlobalAlloc.html).

Кроме того, он отвечает за статистики аллокаций
[`ku::allocator::info::Info`](../../doc/ku/allocator/info/struct.Info.html).
А именно, за полную статистику
[`Dispatcher::info`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.info)
и за статистику аллокатора
[`Dispatcher::fallback`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fallback) ---
[`Dispatcher::fallback_info`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fallback_info).
Это сделано, чтобы не дублировать учёт статистики в каждом из возможных `fallback`.
Также есть статистика у каждого из аллокаторов
[`FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html) ---
[`FixedSizeAllocator::info`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#structfield.info).
За неё они отвечают самостоятельно. Кроме того, она имеет тип
[`ku::allocator::info::Info`](../../doc/ku/allocator/info/struct.Info.html),
а не
[`ku::allocator::info::AtomicInfo`](../../doc/ku/allocator/info/struct.AtomicInfo.html).
Так как работу с каждым из
[`FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html)
диспетчер выполняет под соответствующей спин–блокировкой `Spinlock<FixedSizeAllocator>`.
Эти статистики должны поддерживать такой инвариант:
полная статистика
[`Dispatcher::info`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.info)
равна сумме
[`Dispatcher::fallback_info`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fallback_info)
и всех
[`FixedSizeAllocator::info`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#structfield.info).
Разбиение полной статистики на статистики разных аллокаторов сделано для удобства отладки.

Доработайте метод
[`Dispatcher::alloc()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.alloc)
в файле
[`ku/src/allocator/dispatcher.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/allocator/dispatcher.rs):

```rust
unsafe fn Dispatcher::alloc(
    &self,
    layout: Layout,
) -> *mut u8
```

Сейчас он для любых аллокаций обращается к аллокатору
[`Dispatcher::fallback`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fallback).
Сделайте так, чтобы диспетчер обращался к нему, только когда выделяемый размер:

- Слишком большой и для него нет [`FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html).
- Или же кратен странице, --- в этом случае [`FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html) не выполнял бы никакой полезной работы.

Запрошенный для аллокации размер
[`Layout::size()`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html#method.size)
при этом может быть и чуть меньше, но при округлении должен давать кратный размеру страницы результат.
Обратите внимание, что в
[`Dispatcher::fixed_size`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fixed_size)
может быть запись, которая соответствует такому же размеру.
Но использовать её мы никогда не будем.
Она там есть, только чтобы пересчитывать индексы в этом массиве в размеры аллокаций было удобно.
С другой стороны, в этом массиве нет записи для размера равного нулю,
так как она не сильно упрощает такой пересчёт.

В остальных случаях, выделяйте блок памяти одним из
[`ku::allocator::fixed_size::FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html),
которые собраны в массив
[`Dispatcher::fixed_size`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fixed_size).
Каждый из них отвечает за отдельный размер выделяемых блоков.
Обратите внимание, что
[`FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html)
выделяет блоки размера и выравнивания `size`.
А значит, если
[`Layout::align()`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html#method.size)
больше
[`Layout::size()`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html#method.size),
то ориентироваться нужно на него.
По той же причине `size` должен удовлетворять
[требованиям на `align`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html#method.from_size_align).

Если в нужном аллокаторе фиксированно размера нет свободных блоков,
то диспетчер должен прежде всего выделить блок страниц из
[`Dispatcher::fallback`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fallback).
И отдать их в
[`FixedSizeAllocator::stock_up()`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#method.stock_up)
для нарезки на небольшие блоки.
Определить, есть ли свободные блоки, поможет метод
[`FixedSizeAllocator::is_empty()`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#method.is_empty).
А вычислить подходящий размер большого блока страниц --- метод
[`FixedSizeAllocator::get_pack_layout()`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#method.get_pack_layout).

Удобно вынести часть действий во вспомогательные методы, так как они нам понадобятся неоднократно:

- [`Dispatcher::get_fixed_size_index()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.get_fixed_size_index) возвращает индекс аллокатора фиксированного размера для запрошенного `layout`. Либо `None`, если за `layout` отвечает [`Dispatcher::fallback`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fallback).
- [`Dispatcher::get_size()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.get_size) по индексу в массиве [`Dispatcher::fixed_size`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fixed_size) возвращает размер аллокаций, которые выдаёт соответствующий [`FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html).
- [`Dispatcher::fixed_size_allocate()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.fixed_size_allocate) скрывает логику выделения блока из [`FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html). А именно, --- проверку его непустоты, наполнение блоками, и собственно выделение блока.

Симметрично
[`Dispatcher::alloc()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.alloc)
доработайте метод
[`Dispatcher::dealloc()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.dealloc):

```rust
unsafe fn Dispatcher::dealloc(
    &self,
    ptr: *mut u8,
    layout: Layout,
)
```

Главное, что вы должны помнить --- возвращать блок нужно строго в тот аллокатор, из которого он выделялся.

Аналогично
[`Dispatcher::alloc()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.alloc)
доработайте метод
[`Dispatcher::alloc_zeroed()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.alloc):

```rust
unsafe fn Dispatcher::alloc_zeroed(
    &self,
    layout: Layout,
) -> *mut u8
```

Он должен заполнить нулями выделяемый блок памяти.
Обратите внимание, что из `fallback.allocate_zeroed()`
мы уже получаем заполненный нулями блок, его повторно заполнять не нужно.

Доработайте метод
[`Dispatcher::realloc()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.realloc),
который изменяет размер ранее выделенной памяти:

```rust
unsafe fn Dispatcher::realloc(
    &self,
    ptr: *mut u8,
    layout: Layout,
    new_size: usize,
) -> *mut u8
```

Если и старый размер и новый относятся к
[`Dispatcher::fallback`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fallback),
то нужная логика уже написана.
Но если хотя бы один из них относится к
[`Dispatcher::fixed_size`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#structfield.fixed_size)
и при этом к разным аллокаторам, то аллокацию нужно перенести.
Что-либо делать с аллокацией для которой не меняется выделяемый размер,
даже когда меняется запрошенный
[`Layout::size()`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html#method.size),
не нужно.
При этом можно воспользоваться готовыми вспомогательными методами, а также методами
[`Dispatcher::alloc()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.alloc)
и
[`Dispatcher::dealloc()`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html#method.dealloc).

В типаже
[`core::alloc::GlobalAlloc`](https://doc.rust-lang.org/nightly/core/alloc/trait.GlobalAlloc.html)
нет похожего метода `GlobalAlloc::realloc_zeroed()`.
Так что на этом
[`Dispatcher`](../../doc/ku/allocator/dispatcher/struct.Dispatcher.html)
завершён.

Вам могут пригодиться функции

- [`core::ptr::copy_nonoverlapping()`](https://doc.rust-lang.org/nightly/core/ptr/fn.copy_nonoverlapping.html),
- [`core::ptr::write_bytes()`](https://doc.rust-lang.org/nightly/core/ptr/fn.write_bytes.html).


#### Реализуйте аллокатор блоков одинакового размера [`FixedSizeAllocator`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html)

Реализуйте методы
[`Lifo::pop()`](../../doc/ku/allocator/fixed_size/struct.Lifo.html#method.pop)
и
[`Lifo::push()`](../../doc/ku/allocator/fixed_size/struct.Lifo.html#method.push)
в файле
[`ku/src/allocator/fixed_size.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/allocator/fixed_size.rs):

```rust
fn Lifo::pop(
    &mut self,
) -> Virt

fn Lifo::push(
    &mut self,
    virt: Virt,
)
```

Они предназначены для работы со списком свободных блоков памяти
[`ku::allocator::fixed_size::Lifo`](../../doc/ku/allocator/fixed_size/struct.Lifo.html).
В случае отсутствия свободного блока, метод
[`Lifo::pop()`](../../doc/ku/allocator/fixed_size/struct.Lifo.html#method.pop)
возвращает нулевой адрес
[`Virt::zero()`](../../doc/ku/memory/addr/type.Virt.html#method.zero).

Реализуйте метод
[`FixedSizeAllocator::get_pack_layout()`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#method.get_pack_layout)
в файле
[`ku/src/allocator/fixed_size.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/allocator/fixed_size.rs):

```rust
fn FixedSizeAllocator::get_pack_layout(
    size: usize,
) -> Layout
```

Он возвращает
[`Layout`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html)
блока памяти, который удовлетворяет потенциально противоречащим друг другу требованиям:

- Состоит из целого количества страниц [`Page`](../../doc/ku/memory/frage/type.Page.html).
- Подходит для нарезки на блоки размером `size`.
- Содержит не слишком много страниц по отношению к `size`. Если запросили 16 байт, то запасти заранее 1000 страниц по 4 KiB --- явный перерасход, если же запросили порядка 1MiB, то же количество страниц скорее адекватно.
- Не слишком много памяти теряется на хвостик размера меньше `size`, который останется после нарезки блока страниц на блоки по `size` байт. Если хвостик порядка 2 KiB при выделении одной страницы размера 4KiB --- это слишком расточительно, но такой же по абсолютному размеру хвостик при выделении 1000 страниц вполне приемлем.

Ваша задача --- самостоятельно придумать компромиссные решения для этого метода.

В методе
[`FixedSizeAllocator::stock_up()`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#method.stock_up)
блок с этим
[`Layout`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html)
будет нарезаться на подблоки размера `size`, которые выделяет этот аллокатор.

Реализуйте метод
[`FixedSizeAllocator::stock_up()`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#method.stock_up):

```rust
fn FixedSizeAllocator::stock_up(
    &mut self,
    size: usize,
    pack: Block<Virt>,
)
```

Он режет блок `pack` на подблоки размера `size`, которые сохраняет себе для последующих аллокаций.

Реализуйте метод
[`FixedSizeAllocator::allocate()`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#method.allocate):

```rust
fn FixedSizeAllocator::allocate(
    &mut self,
    layout: Layout,
    size: usize,
) -> *mut u8
```

Он выделяет память из своего списка свободных блоков ---
[`FixedSizeAllocator::free`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#structfield.free).
А запрошенный для аллокации
[`Layout`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html)
и реально выделяемый ей размер `size` понадобятся для учёта этой аллокации в
[`FixedSizeAllocator::info`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#structfield.info).

Реализуйте парный ему метод
[`FixedSizeAllocator::deallocate()`](../../doc/ku/allocator/fixed_size/struct.FixedSizeAllocator.html#method.deallocate):

```rust
fn FixedSizeAllocator::deallocate(
    &mut self,
    ptr: *mut u8,
    layout: Layout,
    size: usize,
)
```


### Проверьте себя

Тесты можно запустить командой `cargo test --test 3-smp-2-small-memory-allocator`
в директориях `kernel` и `ku` репозитория.
В проверочной системе запускаются тесты из обеих этих директорий.

#### Базовый тест из [`kernel/tests/3-smp-2-small-memory-allocator.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/3-smp-2-small-memory-allocator.rs)

Запустите тест `3-smp-2-small-memory-allocator` из файла
[`kernel/tests/3-smp-2-small-memory-allocator.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/3-smp-2-small-memory-allocator.rs):

```console
$ (cd kernel; cargo test --test 3-smp-2-small-memory-allocator)
...
3_smp_2_small_memory_allocator::basic-----------------------
17:27:20 0 D start_info = { allocations: 0 - 0 = 0, requested: 0 B - 0 B = 0 B, allocated: 0 B - 0 B = 0 B, pages: 0 - 0 = 0, loss: 0 B = 0.000% }
17:27:20 0 D box_contents = 2
17:27:20 0 D allocator_info = { total: { allocations: 1 - 0 = 1, requested: 4 B - 0 B = 4 B, allocated: 8 B - 0 B = 8 B, pages: 1 - 0 = 1, loss: 3.996 KiB = 99.902% }, fallback: { allocations: 0 - 0 = 0, requested: 0 B - 0 B = 0 B, allocated: 0 B - 0 B = 0 B, pages: 0 - 0 = 0, loss: 0 B = 0.000% }, size_8: { allocations: 1 - 0 = 1, requested: 4 B - 0 B = 4 B, allocated: 8 B - 0 B = 8 B, pages: 1 - 0 = 1, loss: 3.996 KiB = 99.902% } }
17:27:20 0 D info = { allocations: 1 - 0 = 1, requested: 4 B - 0 B = 4 B, allocated: 8 B - 0 B = 8 B, pages: 1 - 0 = 1, loss: 3.996 KiB = 99.902% }
17:27:20 0 D info_diff = { allocations: 1 - 0 = 1, requested: 4 B - 0 B = 4 B, allocated: 8 B - 0 B = 8 B, pages: 1 - 0 = 1, loss: 3.996 KiB = 99.902% }
17:27:20 0 D allocator_info = { total: { allocations: 1 - 1 = 0, requested: 4 B - 4 B = 0 B, allocated: 8 B - 8 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, fallback: { allocations: 0 - 0 = 0, requested: 0 B - 0 B = 0 B, allocated: 0 B - 0 B = 0 B, pages: 0 - 0 = 0, loss: 0 B = 0.000% }, size_8: { allocations: 1 - 1 = 0, requested: 4 B - 4 B = 0 B, allocated: 8 B - 8 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% } }
17:27:20 0 D end_info = { allocations: 1 - 1 = 0, requested: 4 B - 4 B = 0 B, allocated: 8 B - 8 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }
17:27:20 0 D end_info_diff = { allocations: 1 - 1 = 0, requested: 4 B - 4 B = 0 B, allocated: 8 B - 8 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }
3_smp_2_small_memory_allocator::basic-------------- [passed]

3_smp_2_small_memory_allocator::grow_and_shrink-------------
17:27:20 0 D start_info = { allocations: 1 - 1 = 0, requested: 4 B - 4 B = 0 B, allocated: 8 B - 8 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }
17:27:20 0 D contents_sum = 75491328; push_sum = 75491328
17:27:20 0 D allocator_info = { total: { allocations: 14 - 13 = 1, requested: 255.973 KiB - 127.973 KiB = 128.000 KiB, allocated: 255.977 KiB - 127.977 KiB = 128.000 KiB, pages: 72 - 31 = 41, loss: 36.000 KiB = 21.951% }, fallback: { allocations: 6 - 5 = 1, requested: 252.000 KiB - 124.000 KiB = 128.000 KiB, allocated: 252.000 KiB - 124.000 KiB = 128.000 KiB, pages: 63 - 31 = 32, loss: 0 B = 0.000% }, size_8: { allocations: 1 - 1 = 0, requested: 4 B - 4 B = 0 B, allocated: 8 B - 8 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_32: { allocations: 1 - 1 = 0, requested: 32 B - 32 B = 0 B, allocated: 32 B - 32 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_64: { allocations: 1 - 1 = 0, requested: 64 B - 64 B = 0 B, allocated: 64 B - 64 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_128: { allocations: 1 - 1 = 0, requested: 128 B - 128 B = 0 B, allocated: 128 B - 128 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_256: { allocations: 1 - 1 = 0, requested: 256 B - 256 B = 0 B, allocated: 256 B - 256 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_512: { allocations: 1 - 1 = 0, requested: 512 B - 512 B = 0 B, allocated: 512 B - 512 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_1024: { allocations: 1 - 1 = 0, requested: 1.000 KiB - 1.000 KiB = 0 B, allocated: 1.000 KiB - 1.000 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_2048: { allocations: 1 - 1 = 0, requested: 2.000 KiB - 2.000 KiB = 0 B, allocated: 2.000 KiB - 2.000 KiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% } }
17:27:20 0 D info = { allocations: 14 - 13 = 1, requested: 255.973 KiB - 127.973 KiB = 128.000 KiB, allocated: 255.977 KiB - 127.977 KiB = 128.000 KiB, pages: 72 - 31 = 41, loss: 36.000 KiB = 21.951% }
17:27:20 0 D info_diff = { allocations: 13 - 12 = 1, requested: 255.969 KiB - 127.969 KiB = 128.000 KiB, allocated: 255.969 KiB - 127.969 KiB = 128.000 KiB, pages: 71 - 31 = 40, loss: 32.000 KiB = 20.000% }
17:27:22 0 D contents_sum = 75491328; pop_sum = 75491328
17:27:22 0 D allocator_info = { total: { allocations: 28 - 28 = 0, requested: 383.965 KiB - 383.965 KiB = 0 B, allocated: 383.969 KiB - 383.969 KiB = 0 B, pages: 104 - 94 = 10, loss: 40.000 KiB = 100.000% }, fallback: { allocations: 11 - 11 = 0, requested: 376.000 KiB - 376.000 KiB = 0 B, allocated: 376.000 KiB - 376.000 KiB = 0 B, pages: 94 - 94 = 0, loss: 0 B = 0.000% }, size_8: { allocations: 2 - 2 = 0, requested: 12 B - 12 B = 0 B, allocated: 16 B - 16 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_16: { allocations: 1 - 1 = 0, requested: 16 B - 16 B = 0 B, allocated: 16 B - 16 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_32: { allocations: 2 - 2 = 0, requested: 64 B - 64 B = 0 B, allocated: 64 B - 64 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_64: { allocations: 2 - 2 = 0, requested: 128 B - 128 B = 0 B, allocated: 128 B - 128 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_128: { allocations: 2 - 2 = 0, requested: 256 B - 256 B = 0 B, allocated: 256 B - 256 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_256: { allocations: 2 - 2 = 0, requested: 512 B - 512 B = 0 B, allocated: 512 B - 512 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_512: { allocations: 2 - 2 = 0, requested: 1.000 KiB - 1.000 KiB = 0 B, allocated: 1.000 KiB - 1.000 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_1024: { allocations: 2 - 2 = 0, requested: 2.000 KiB - 2.000 KiB = 0 B, allocated: 2.000 KiB - 2.000 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_2048: { allocations: 2 - 2 = 0, requested: 4.000 KiB - 4.000 KiB = 0 B, allocated: 4.000 KiB - 4.000 KiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% } }
17:27:22 0 D end_info = { allocations: 28 - 28 = 0, requested: 383.965 KiB - 383.965 KiB = 0 B, allocated: 383.969 KiB - 383.969 KiB = 0 B, pages: 104 - 94 = 10, loss: 40.000 KiB = 100.000% }
3_smp_2_small_memory_allocator::grow_and_shrink---- [passed]

3_smp_2_small_memory_allocator::stress----------------------
17:27:22 0 D start_info = { allocations: 28 - 28 = 0, requested: 383.965 KiB - 383.965 KiB = 0 B, allocated: 383.969 KiB - 383.969 KiB = 0 B, pages: 104 - 94 = 10, loss: 40.000 KiB = 100.000% }
17:27:22 0 D vector_length = 0
17:27:22 0 D vector_length = 5000
17:27:23.147 0 D vector_length = 10000
17:27:23.587 0 D vector_length = 15000
17:27:24.043 0 D vector_length = 20000
17:27:24.483 0 D vector_length = 25000
17:27:24.921 0 D vector_length = 30000
17:27:25.395 0 D vector_length = 35000
17:27:25.833 0 D vector_length = 40000
17:27:26.267 0 D vector_length = 45000
17:27:26.729 0 D vector_length = 50000
17:27:27.187 0 D vector_length = 55000
17:27:27.647 0 D vector_length = 60000
17:27:28.103 0 D vector_length = 65000
17:27:28.641 0 D vector_length = 70000
17:27:29.099 0 D vector_length = 75000
17:27:29.561 0 D vector_length = 80000
17:27:30.021 0 D vector_length = 85000
17:27:30.483 0 D vector_length = 90000
17:27:30.943 0 D vector_length = 95000
17:27:31.419 0 D contents_sum = 333328333350000; push_sum = 333328333350000
17:27:31.421 0 D allocator_info = { total: { allocations: 100077 - 43 = 100034, requested: 3.144 MiB - 1.375 MiB = 1.769 MiB, allocated: 3.144 MiB - 1.375 MiB = 1.769 MiB, pages: 813 - 349 = 464, loss: 44.094 KiB = 2.376% }, fallback: { allocations: 20 - 19 = 1, requested: 2.363 MiB - 1.363 MiB = 1.000 MiB, allocated: 2.363 MiB - 1.363 MiB = 1.000 MiB, pages: 605 - 349 = 256, loss: 0 B = 0.000% }, size_8: { allocations: 100002 - 2 = 100000, requested: 781.262 KiB - 12 B = 781.250 KiB, allocated: 781.266 KiB - 16 B = 781.250 KiB, pages: 196 - 0 = 196, loss: 2.750 KiB = 0.351% }, size_16: { allocations: 1 - 1 = 0, requested: 16 B - 16 B = 0 B, allocated: 16 B - 16 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_32: { allocations: 3 - 3 = 0, requested: 96 B - 96 B = 0 B, allocated: 96 B - 96 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_64: { allocations: 3 - 3 = 0, requested: 192 B - 192 B = 0 B, allocated: 192 B - 192 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_128: { allocations: 3 - 3 = 0, requested: 384 B - 384 B = 0 B, allocated: 384 B - 384 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_192: { allocations: 28 - 0 = 28, requested: 5.250 KiB - 0 B = 5.250 KiB, allocated: 5.250 KiB - 0 B = 5.250 KiB, pages: 2 - 0 = 2, loss: 2.750 KiB = 34.375% }, size_256: { allocations: 3 - 3 = 0, requested: 768 B - 768 B = 0 B, allocated: 768 B - 768 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_288: { allocations: 5 - 0 = 5, requested: 1.406 KiB - 0 B = 1.406 KiB, allocated: 1.406 KiB - 0 B = 1.406 KiB, pages: 1 - 0 = 1, loss: 2.594 KiB = 64.844% }, size_512: { allocations: 3 - 3 = 0, requested: 1.500 KiB - 1.500 KiB = 0 B, allocated: 1.500 KiB - 1.500 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_1024: { allocations: 3 - 3 = 0, requested: 3.000 KiB - 3.000 KiB = 0 B, allocated: 3.000 KiB - 3.000 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_2048: { allocations: 3 - 3 = 0, requested: 6.000 KiB - 6.000 KiB = 0 B, allocated: 6.000 KiB - 6.000 KiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% } }
17:27:31.811 0 D info = { allocations: 100077 - 43 = 100034, requested: 3.144 MiB - 1.375 MiB = 1.769 MiB, allocated: 3.144 MiB - 1.375 MiB = 1.769 MiB, pages: 813 - 349 = 464, loss: 44.094 KiB = 2.376% }
17:27:31.821 0 D info_diff = { allocations: 100049 - 15 = 100034, requested: 2.769 MiB - 1023.969 KiB = 1.769 MiB, allocated: 2.769 MiB - 1023.969 KiB = 1.769 MiB, pages: 709 - 255 = 454, loss: 4.094 KiB = 0.225% }
17:27:32.397 0 D vector_length = 95000
17:27:32.963 0 D vector_length = 90000
17:27:33.527 0 D vector_length = 85000
17:27:34.093 0 D vector_length = 80000
17:27:34.661 0 D vector_length = 75000
17:27:35.225 0 D vector_length = 70000
17:27:35.811 0 D vector_length = 65000
17:27:36.373 0 D vector_length = 60000
17:27:36.937 0 D vector_length = 55000
17:27:37.501 0 D vector_length = 50000
17:27:38.063 0 D vector_length = 45000
17:27:38.629 0 D vector_length = 40000
17:27:39.193 0 D vector_length = 35000
17:27:39.767 0 D vector_length = 30000
17:27:40.331 0 D vector_length = 25000
17:27:40.893 0 D vector_length = 20000
17:27:41.465 0 D vector_length = 15000
17:27:42.033 0 D vector_length = 10000
17:27:42.601 0 D vector_length = 5000
17:27:43.167 0 D vector_length = 0
17:27:43.171 0 D contents_sum = 333328333350000; pop_sum = 333328333350000
17:27:43.183 0 D allocator_info = { total: { allocations: 100094 - 100094 = 0, requested: 4.144 MiB - 4.144 MiB = 0 B, allocated: 4.144 MiB - 4.144 MiB = 0 B, pages: 1068 - 860 = 208, loss: 832.000 KiB = 100.000% }, fallback: { allocations: 28 - 28 = 0, requested: 3.359 MiB - 3.359 MiB = 0 B, allocated: 3.359 MiB - 3.359 MiB = 0 B, pages: 860 - 860 = 0, loss: 0 B = 0.000% }, size_8: { allocations: 100003 - 100003 = 0, requested: 781.270 KiB - 781.270 KiB = 0 B, allocated: 781.273 KiB - 781.273 KiB = 0 B, pages: 196 - 0 = 196, loss: 784.000 KiB = 100.000% }, size_16: { allocations: 2 - 2 = 0, requested: 32 B - 32 B = 0 B, allocated: 32 B - 32 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_32: { allocations: 4 - 4 = 0, requested: 128 B - 128 B = 0 B, allocated: 128 B - 128 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_64: { allocations: 4 - 4 = 0, requested: 256 B - 256 B = 0 B, allocated: 256 B - 256 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_128: { allocations: 4 - 4 = 0, requested: 512 B - 512 B = 0 B, allocated: 512 B - 512 B = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_192: { allocations: 28 - 28 = 0, requested: 5.250 KiB - 5.250 KiB = 0 B, allocated: 5.250 KiB - 5.250 KiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% }, size_256: { allocations: 4 - 4 = 0, requested: 1.000 KiB - 1.000 KiB = 0 B, allocated: 1.000 KiB - 1.000 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_288: { allocations: 5 - 5 = 0, requested: 1.406 KiB - 1.406 KiB = 0 B, allocated: 1.406 KiB - 1.406 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_512: { allocations: 4 - 4 = 0, requested: 2.000 KiB - 2.000 KiB = 0 B, allocated: 2.000 KiB - 2.000 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_1024: { allocations: 4 - 4 = 0, requested: 4.000 KiB - 4.000 KiB = 0 B, allocated: 4.000 KiB - 4.000 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% }, size_2048: { allocations: 4 - 4 = 0, requested: 8.000 KiB - 8.000 KiB = 0 B, allocated: 8.000 KiB - 8.000 KiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% } }
17:27:43.571 0 D end_info = { allocations: 100094 - 100094 = 0, requested: 4.144 MiB - 4.144 MiB = 0 B, allocated: 4.144 MiB - 4.144 MiB = 0 B, pages: 1068 - 860 = 208, loss: 832.000 KiB = 100.000% }
17:27:43.581 0 D values = 100000; pages_for_values = 196
3_smp_2_small_memory_allocator::stress------------- [passed]
17:27:43.587 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


#### Стресс--тест из [`ku/tests/3-smp-2-small-memory-allocator.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/3-smp-2-small-memory-allocator.rs)

Также запустите тест `3-smp-2-small-memory-allocator` из файла
[`ku/tests/3-smp-2-small-memory-allocator.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/3-smp-2-small-memory-allocator.rs):

```console
$ (cd ku; cargo test --test 3-smp-2-small-memory-allocator)
...
running 1 test
test single_threaded ... 2023-11-05T14:01:37.836215Z i single_threaded iteration=200000 allocation={ size: 817, align: 256, ptr: 0v7F6A_533E_F000, sum: 105028 }
2023-11-05T14:01:42.784994Z i single_threaded iteration=400000 allocation={ size: 667, align: 2, ptr: 0v7F6A_533B_A2E0, sum: 82779 }
2023-11-05T14:01:47.716638Z i single_threaded iteration=600000 allocation={ size: 353, align: 16, ptr: 0v7F6A_5344_2760, sum: 45104 }
2023-11-05T14:01:52.664391Z i single_threaded iteration=800000 allocation={ size: 280, align: 4096, ptr: 0v7F6A_5336_D000, sum: 35423 }
2023-11-05T14:01:57.608528Z i single_threaded iteration=1000000 allocation={ size: 554, align: 2048, ptr: 0v7F6A_5339_3000, sum: 69837 }
2023-11-05T14:02:02.564852Z i single_threaded iteration=1200000 allocation={ size: 12, align: 512, ptr: 0v7F6A_5330_F600, sum: 1462 }
2023-11-05T14:02:07.524710Z i single_threaded iteration=1400000 allocation={ size: 945, align: 128, ptr: 0v7F6A_5322_2400, sum: 119906 }
2023-11-05T14:02:12.466244Z i single_threaded iteration=1600000 allocation={ size: 856, align: 32, ptr: 0v7F6A_533D_E940, sum: 108071 }
2023-11-05T14:02:17.412034Z i single_threaded iteration=1800000 allocation={ size: 152, align: 2048, ptr: 0v7F6A_532E_1800, sum: 20446 }
2023-11-05T14:02:22.344946Z i single_threaded iteration=2000000 allocation={ size: 134, align: 2, ptr: 0v7F6A_5341_3230, sum: 16684 }
2023-11-05T14:02:22.359880Z i single_threaded total={ allocations: 1333487 - 1333487 = 0, requested: 651.684 MiB - 651.684 MiB = 0 B, allocated: 1.222 GiB - 1.222 GiB = 0 B, pages: 102629 - 102052 = 577, loss: 2.254 MiB = 100.000% }
{
    valid: true,
    total: { allocations: 1333487 - 1333487 = 0, requested: 651.684 MiB - 651.684 MiB = 0 B, allocated: 1.222 GiB - 1.222 GiB = 0 B, pages: 102629 - 102052 = 577, loss: 2.254 MiB = 100.000% },
    fallback: { allocations: 102052 - 102052 = 0, requested: 49.932 MiB - 49.932 MiB = 0 B, allocated: 398.641 MiB - 398.641 MiB = 0 B, pages: 102052 - 102052 = 0, loss: 0 B = 0.000% },
    size_8: { allocations: 3228 - 3228 = 0, requested: 14.387 KiB - 14.387 KiB = 0 B, allocated: 25.219 KiB - 25.219 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_16: { allocations: 4686 - 4686 = 0, requested: 51.269 KiB - 51.269 KiB = 0 B, allocated: 73.219 KiB - 73.219 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_24: { allocations: 3136 - 3136 = 0, requested: 63.004 KiB - 63.004 KiB = 0 B, allocated: 73.500 KiB - 73.500 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_32: { allocations: 7841 - 7841 = 0, requested: 174.232 KiB - 174.232 KiB = 0 B, allocated: 245.031 KiB - 245.031 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_40: { allocations: 3221 - 3221 = 0, requested: 114.764 KiB - 114.764 KiB = 0 B, allocated: 125.820 KiB - 125.820 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_48: { allocations: 4858 - 4858 = 0, requested: 205.004 KiB - 205.004 KiB = 0 B, allocated: 227.719 KiB - 227.719 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_56: { allocations: 3227 - 3227 = 0, requested: 165.651 KiB - 165.651 KiB = 0 B, allocated: 176.477 KiB - 176.477 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_64: { allocations: 14406 - 14406 = 0, requested: 633.982 KiB - 633.982 KiB = 0 B, allocated: 900.375 KiB - 900.375 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_72: { allocations: 3237 - 3237 = 0, requested: 216.602 KiB - 216.602 KiB = 0 B, allocated: 227.602 KiB - 227.602 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_80: { allocations: 4812 - 4812 = 0, requested: 353.071 KiB - 353.071 KiB = 0 B, allocated: 375.938 KiB - 375.938 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_88: { allocations: 3197 - 3197 = 0, requested: 263.921 KiB - 263.921 KiB = 0 B, allocated: 274.742 KiB - 274.742 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_96: { allocations: 8181 - 8181 = 0, requested: 694.121 KiB - 694.121 KiB = 0 B, allocated: 766.969 KiB - 766.969 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_104: { allocations: 3332 - 3332 = 0, requested: 327.088 KiB - 327.088 KiB = 0 B, allocated: 338.406 KiB - 338.406 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_112: { allocations: 4735 - 4735 = 0, requested: 495.388 KiB - 495.388 KiB = 0 B, allocated: 517.891 KiB - 517.891 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_120: { allocations: 3232 - 3232 = 0, requested: 367.537 KiB - 367.537 KiB = 0 B, allocated: 378.750 KiB - 378.750 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_128: { allocations: 27235 - 27235 = 0, requested: 2.278 MiB - 2.278 MiB = 0 B, allocated: 3.325 MiB - 3.325 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_136: { allocations: 3217 - 3217 = 0, requested: 416.132 KiB - 416.132 KiB = 0 B, allocated: 427.258 KiB - 427.258 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_144: { allocations: 4807 - 4807 = 0, requested: 653.482 KiB - 653.482 KiB = 0 B, allocated: 675.984 KiB - 675.984 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_152: { allocations: 3260 - 3260 = 0, requested: 472.921 KiB - 472.921 KiB = 0 B, allocated: 483.906 KiB - 483.906 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_160: { allocations: 7979 - 7979 = 0, requested: 1.148 MiB - 1.148 MiB = 0 B, allocated: 1.217 MiB - 1.217 MiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_168: { allocations: 3199 - 3199 = 0, requested: 513.990 KiB - 513.990 KiB = 0 B, allocated: 524.836 KiB - 524.836 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_176: { allocations: 4783 - 4783 = 0, requested: 799.437 KiB - 799.437 KiB = 0 B, allocated: 822.078 KiB - 822.078 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_184: { allocations: 3128 - 3128 = 0, requested: 551.288 KiB - 551.288 KiB = 0 B, allocated: 562.062 KiB - 562.062 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_192: { allocations: 14391 - 14391 = 0, requested: 2.374 MiB - 2.374 MiB = 0 B, allocated: 2.635 MiB - 2.635 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_200: { allocations: 3342 - 3342 = 0, requested: 641.387 KiB - 641.387 KiB = 0 B, allocated: 652.734 KiB - 652.734 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_208: { allocations: 4827 - 4827 = 0, requested: 957.466 KiB - 957.466 KiB = 0 B, allocated: 980.484 KiB - 980.484 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_216: { allocations: 3107 - 3107 = 0, requested: 644.736 KiB - 644.736 KiB = 0 B, allocated: 655.383 KiB - 655.383 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_224: { allocations: 8085 - 8085 = 0, requested: 1.656 MiB - 1.656 MiB = 0 B, allocated: 1.727 MiB - 1.727 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_232: { allocations: 3257 - 3257 = 0, requested: 726.947 KiB - 726.947 KiB = 0 B, allocated: 737.914 KiB - 737.914 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_240: { allocations: 4680 - 4680 = 0, requested: 1.049 MiB - 1.049 MiB = 0 B, allocated: 1.071 MiB - 1.071 MiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_248: { allocations: 3118 - 3118 = 0, requested: 744.484 KiB - 744.484 KiB = 0 B, allocated: 755.141 KiB - 755.141 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_256: { allocations: 52851 - 52851 = 0, requested: 8.779 MiB - 8.779 MiB = 0 B, allocated: 12.903 MiB - 12.903 MiB = 0 B, pages: 7 - 0 = 7, loss: 28.000 KiB = 100.000% },
    size_264: { allocations: 3248 - 3248 = 0, requested: 826.234 KiB - 826.234 KiB = 0 B, allocated: 837.375 KiB - 837.375 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_272: { allocations: 4725 - 4725 = 0, requested: 1.204 MiB - 1.204 MiB = 0 B, allocated: 1.226 MiB - 1.226 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_280: { allocations: 3230 - 3230 = 0, requested: 872.157 KiB - 872.157 KiB = 0 B, allocated: 883.203 KiB - 883.203 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_288: { allocations: 7978 - 7978 = 0, requested: 2.123 MiB - 2.123 MiB = 0 B, allocated: 2.191 MiB - 2.191 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_296: { allocations: 3181 - 3181 = 0, requested: 908.487 KiB - 908.487 KiB = 0 B, allocated: 919.508 KiB - 919.508 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_304: { allocations: 4734 - 4734 = 0, requested: 1.351 MiB - 1.351 MiB = 0 B, allocated: 1.372 MiB - 1.372 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_312: { allocations: 3218 - 3218 = 0, requested: 969.502 KiB - 969.502 KiB = 0 B, allocated: 980.484 KiB - 980.484 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_320: { allocations: 14345 - 14345 = 0, requested: 4.119 MiB - 4.119 MiB = 0 B, allocated: 4.378 MiB - 4.378 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_328: { allocations: 3104 - 3104 = 0, requested: 983.652 KiB - 983.652 KiB = 0 B, allocated: 994.250 KiB - 994.250 KiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_336: { allocations: 4860 - 4860 = 0, requested: 1.535 MiB - 1.535 MiB = 0 B, allocated: 1.557 MiB - 1.557 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_344: { allocations: 3174 - 3174 = 0, requested: 1.031 MiB - 1.031 MiB = 0 B, allocated: 1.041 MiB - 1.041 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_352: { allocations: 7999 - 7999 = 0, requested: 2.616 MiB - 2.616 MiB = 0 B, allocated: 2.685 MiB - 2.685 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_360: { allocations: 3223 - 3223 = 0, requested: 1.096 MiB - 1.096 MiB = 0 B, allocated: 1.107 MiB - 1.107 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_368: { allocations: 4711 - 4711 = 0, requested: 1.631 MiB - 1.631 MiB = 0 B, allocated: 1.653 MiB - 1.653 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_376: { allocations: 3191 - 3191 = 0, requested: 1.134 MiB - 1.134 MiB = 0 B, allocated: 1.144 MiB - 1.144 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_384: { allocations: 27365 - 27365 = 0, requested: 8.975 MiB - 8.975 MiB = 0 B, allocated: 10.021 MiB - 10.021 MiB = 0 B, pages: 7 - 0 = 7, loss: 28.000 KiB = 100.000% },
    size_392: { allocations: 3164 - 3164 = 0, requested: 1.172 MiB - 1.172 MiB = 0 B, allocated: 1.183 MiB - 1.183 MiB = 0 B, pages: 1 - 0 = 1, loss: 4.000 KiB = 100.000% },
    size_400: { allocations: 4688 - 4688 = 0, requested: 1.767 MiB - 1.767 MiB = 0 B, allocated: 1.788 MiB - 1.788 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_408: { allocations: 3145 - 3145 = 0, requested: 1.213 MiB - 1.213 MiB = 0 B, allocated: 1.224 MiB - 1.224 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_416: { allocations: 7948 - 7948 = 0, requested: 3.084 MiB - 3.084 MiB = 0 B, allocated: 3.153 MiB - 3.153 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_424: { allocations: 3212 - 3212 = 0, requested: 1.288 MiB - 1.288 MiB = 0 B, allocated: 1.299 MiB - 1.299 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_432: { allocations: 4786 - 4786 = 0, requested: 1.950 MiB - 1.950 MiB = 0 B, allocated: 1.972 MiB - 1.972 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_440: { allocations: 3215 - 3215 = 0, requested: 1.338 MiB - 1.338 MiB = 0 B, allocated: 1.349 MiB - 1.349 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_448: { allocations: 14476 - 14476 = 0, requested: 5.920 MiB - 5.920 MiB = 0 B, allocated: 6.185 MiB - 6.185 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_456: { allocations: 3162 - 3162 = 0, requested: 1.365 MiB - 1.365 MiB = 0 B, allocated: 1.375 MiB - 1.375 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_464: { allocations: 4771 - 4771 = 0, requested: 2.089 MiB - 2.089 MiB = 0 B, allocated: 2.111 MiB - 2.111 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_472: { allocations: 3254 - 3254 = 0, requested: 1.454 MiB - 1.454 MiB = 0 B, allocated: 1.465 MiB - 1.465 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_480: { allocations: 8248 - 8248 = 0, requested: 3.704 MiB - 3.704 MiB = 0 B, allocated: 3.776 MiB - 3.776 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_488: { allocations: 3167 - 3167 = 0, requested: 1.463 MiB - 1.463 MiB = 0 B, allocated: 1.474 MiB - 1.474 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_496: { allocations: 4881 - 4881 = 0, requested: 2.286 MiB - 2.286 MiB = 0 B, allocated: 2.309 MiB - 2.309 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_504: { allocations: 3127 - 3127 = 0, requested: 1.492 MiB - 1.492 MiB = 0 B, allocated: 1.503 MiB - 1.503 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_512: { allocations: 105126 - 105126 = 0, requested: 34.573 MiB - 34.573 MiB = 0 B, allocated: 51.331 MiB - 51.331 MiB = 0 B, pages: 23 - 0 = 23, loss: 92.000 KiB = 100.000% },
    size_520: { allocations: 3152 - 3152 = 0, requested: 1.553 MiB - 1.553 MiB = 0 B, allocated: 1.563 MiB - 1.563 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_528: { allocations: 4774 - 4774 = 0, requested: 2.382 MiB - 2.382 MiB = 0 B, allocated: 2.404 MiB - 2.404 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_536: { allocations: 3109 - 3109 = 0, requested: 1.579 MiB - 1.579 MiB = 0 B, allocated: 1.589 MiB - 1.589 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_544: { allocations: 8139 - 8139 = 0, requested: 4.150 MiB - 4.150 MiB = 0 B, allocated: 4.223 MiB - 4.223 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_552: { allocations: 3322 - 3322 = 0, requested: 1.738 MiB - 1.738 MiB = 0 B, allocated: 1.749 MiB - 1.749 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_560: { allocations: 4786 - 4786 = 0, requested: 2.534 MiB - 2.534 MiB = 0 B, allocated: 2.556 MiB - 2.556 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_568: { allocations: 3201 - 3201 = 0, requested: 1.723 MiB - 1.723 MiB = 0 B, allocated: 1.734 MiB - 1.734 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_576: { allocations: 14676 - 14676 = 0, requested: 7.798 MiB - 7.798 MiB = 0 B, allocated: 8.062 MiB - 8.062 MiB = 0 B, pages: 6 - 0 = 6, loss: 24.000 KiB = 100.000% },
    size_584: { allocations: 3269 - 3269 = 0, requested: 1.810 MiB - 1.810 MiB = 0 B, allocated: 1.821 MiB - 1.821 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_592: { allocations: 4829 - 4829 = 0, requested: 2.704 MiB - 2.704 MiB = 0 B, allocated: 2.726 MiB - 2.726 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_600: { allocations: 3155 - 3155 = 0, requested: 1.795 MiB - 1.795 MiB = 0 B, allocated: 1.805 MiB - 1.805 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_608: { allocations: 8120 - 8120 = 0, requested: 4.638 MiB - 4.638 MiB = 0 B, allocated: 4.708 MiB - 4.708 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_616: { allocations: 3158 - 3158 = 0, requested: 1.845 MiB - 1.845 MiB = 0 B, allocated: 1.855 MiB - 1.855 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_624: { allocations: 4835 - 4835 = 0, requested: 2.855 MiB - 2.855 MiB = 0 B, allocated: 2.877 MiB - 2.877 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_632: { allocations: 3210 - 3210 = 0, requested: 1.924 MiB - 1.924 MiB = 0 B, allocated: 1.935 MiB - 1.935 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_640: { allocations: 27145 - 27145 = 0, requested: 15.530 MiB - 15.530 MiB = 0 B, allocated: 16.568 MiB - 16.568 MiB = 0 B, pages: 10 - 0 = 10, loss: 40.000 KiB = 100.000% },
    size_648: { allocations: 3074 - 3074 = 0, requested: 1.889 MiB - 1.889 MiB = 0 B, allocated: 1.900 MiB - 1.900 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_656: { allocations: 4854 - 4854 = 0, requested: 3.015 MiB - 3.015 MiB = 0 B, allocated: 3.037 MiB - 3.037 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_664: { allocations: 3252 - 3252 = 0, requested: 2.049 MiB - 2.049 MiB = 0 B, allocated: 2.059 MiB - 2.059 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_672: { allocations: 7901 - 7901 = 0, requested: 4.994 MiB - 4.994 MiB = 0 B, allocated: 5.064 MiB - 5.064 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_680: { allocations: 3096 - 3096 = 0, requested: 1.997 MiB - 1.997 MiB = 0 B, allocated: 2.008 MiB - 2.008 MiB = 0 B, pages: 2 - 0 = 2, loss: 8.000 KiB = 100.000% },
    size_688: { allocations: 4785 - 4785 = 0, requested: 3.117 MiB - 3.117 MiB = 0 B, allocated: 3.140 MiB - 3.140 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_696: { allocations: 3185 - 3185 = 0, requested: 2.103 MiB - 2.103 MiB = 0 B, allocated: 2.114 MiB - 2.114 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_704: { allocations: 14516 - 14516 = 0, requested: 9.484 MiB - 9.484 MiB = 0 B, allocated: 9.746 MiB - 9.746 MiB = 0 B, pages: 8 - 0 = 8, loss: 32.000 KiB = 100.000% },
    size_712: { allocations: 3240 - 3240 = 0, requested: 2.189 MiB - 2.189 MiB = 0 B, allocated: 2.200 MiB - 2.200 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_720: { allocations: 4729 - 4729 = 0, requested: 3.225 MiB - 3.225 MiB = 0 B, allocated: 3.247 MiB - 3.247 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_728: { allocations: 3097 - 3097 = 0, requested: 2.140 MiB - 2.140 MiB = 0 B, allocated: 2.150 MiB - 2.150 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_736: { allocations: 8064 - 8064 = 0, requested: 5.590 MiB - 5.590 MiB = 0 B, allocated: 5.660 MiB - 5.660 MiB = 0 B, pages: 5 - 0 = 5, loss: 20.000 KiB = 100.000% },
    size_744: { allocations: 3082 - 3082 = 0, requested: 2.177 MiB - 2.177 MiB = 0 B, allocated: 2.187 MiB - 2.187 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_752: { allocations: 4777 - 4777 = 0, requested: 3.403 MiB - 3.403 MiB = 0 B, allocated: 3.426 MiB - 3.426 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_760: { allocations: 3285 - 3285 = 0, requested: 2.370 MiB - 2.370 MiB = 0 B, allocated: 2.381 MiB - 2.381 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_768: { allocations: 52982 - 52982 = 0, requested: 34.646 MiB - 34.646 MiB = 0 B, allocated: 38.805 MiB - 38.805 MiB = 0 B, pages: 22 - 0 = 22, loss: 88.000 KiB = 100.000% },
    size_776: { allocations: 3183 - 3183 = 0, requested: 2.345 MiB - 2.345 MiB = 0 B, allocated: 2.356 MiB - 2.356 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_784: { allocations: 4754 - 4754 = 0, requested: 3.532 MiB - 3.532 MiB = 0 B, allocated: 3.554 MiB - 3.554 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_792: { allocations: 3131 - 3131 = 0, requested: 2.354 MiB - 2.354 MiB = 0 B, allocated: 2.365 MiB - 2.365 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_800: { allocations: 8119 - 8119 = 0, requested: 6.124 MiB - 6.124 MiB = 0 B, allocated: 6.194 MiB - 6.194 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_808: { allocations: 3159 - 3159 = 0, requested: 2.424 MiB - 2.424 MiB = 0 B, allocated: 2.434 MiB - 2.434 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_816: { allocations: 4818 - 4818 = 0, requested: 3.728 MiB - 3.728 MiB = 0 B, allocated: 3.749 MiB - 3.749 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_824: { allocations: 3230 - 3230 = 0, requested: 2.527 MiB - 2.527 MiB = 0 B, allocated: 2.538 MiB - 2.538 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_832: { allocations: 14577 - 14577 = 0, requested: 11.302 MiB - 11.302 MiB = 0 B, allocated: 11.566 MiB - 11.566 MiB = 0 B, pages: 9 - 0 = 9, loss: 36.000 KiB = 100.000% },
    size_840: { allocations: 3235 - 3235 = 0, requested: 2.581 MiB - 2.581 MiB = 0 B, allocated: 2.592 MiB - 2.592 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_848: { allocations: 4860 - 4860 = 0, requested: 3.908 MiB - 3.908 MiB = 0 B, allocated: 3.930 MiB - 3.930 MiB = 0 B, pages: 6 - 0 = 6, loss: 24.000 KiB = 100.000% },
    size_856: { allocations: 3230 - 3230 = 0, requested: 2.626 MiB - 2.626 MiB = 0 B, allocated: 2.637 MiB - 2.637 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_864: { allocations: 8190 - 8190 = 0, requested: 6.678 MiB - 6.678 MiB = 0 B, allocated: 6.748 MiB - 6.748 MiB = 0 B, pages: 6 - 0 = 6, loss: 24.000 KiB = 100.000% },
    size_872: { allocations: 3176 - 3176 = 0, requested: 2.630 MiB - 2.630 MiB = 0 B, allocated: 2.641 MiB - 2.641 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_880: { allocations: 4878 - 4878 = 0, requested: 4.071 MiB - 4.071 MiB = 0 B, allocated: 4.094 MiB - 4.094 MiB = 0 B, pages: 5 - 0 = 5, loss: 20.000 KiB = 100.000% },
    size_888: { allocations: 3169 - 3169 = 0, requested: 2.673 MiB - 2.673 MiB = 0 B, allocated: 2.684 MiB - 2.684 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_896: { allocations: 27253 - 27253 = 0, requested: 22.254 MiB - 22.254 MiB = 0 B, allocated: 23.287 MiB - 23.287 MiB = 0 B, pages: 15 - 0 = 15, loss: 60.000 KiB = 100.000% },
    size_904: { allocations: 3098 - 3098 = 0, requested: 2.660 MiB - 2.660 MiB = 0 B, allocated: 2.671 MiB - 2.671 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_912: { allocations: 4835 - 4835 = 0, requested: 4.183 MiB - 4.183 MiB = 0 B, allocated: 4.205 MiB - 4.205 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_920: { allocations: 3141 - 3141 = 0, requested: 2.745 MiB - 2.745 MiB = 0 B, allocated: 2.756 MiB - 2.756 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_928: { allocations: 8017 - 8017 = 0, requested: 7.026 MiB - 7.026 MiB = 0 B, allocated: 7.095 MiB - 7.095 MiB = 0 B, pages: 6 - 0 = 6, loss: 24.000 KiB = 100.000% },
    size_936: { allocations: 3097 - 3097 = 0, requested: 2.754 MiB - 2.754 MiB = 0 B, allocated: 2.765 MiB - 2.765 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_944: { allocations: 4766 - 4766 = 0, requested: 4.269 MiB - 4.269 MiB = 0 B, allocated: 4.291 MiB - 4.291 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_952: { allocations: 3169 - 3169 = 0, requested: 2.867 MiB - 2.867 MiB = 0 B, allocated: 2.877 MiB - 2.877 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_960: { allocations: 14419 - 14419 = 0, requested: 12.940 MiB - 12.940 MiB = 0 B, allocated: 13.201 MiB - 13.201 MiB = 0 B, pages: 10 - 0 = 10, loss: 40.000 KiB = 100.000% },
    size_968: { allocations: 3170 - 3170 = 0, requested: 2.916 MiB - 2.916 MiB = 0 B, allocated: 2.926 MiB - 2.926 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_976: { allocations: 4790 - 4790 = 0, requested: 4.436 MiB - 4.436 MiB = 0 B, allocated: 4.458 MiB - 4.458 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_984: { allocations: 3209 - 3209 = 0, requested: 3.000 MiB - 3.000 MiB = 0 B, allocated: 3.011 MiB - 3.011 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_992: { allocations: 8045 - 8045 = 0, requested: 7.541 MiB - 7.541 MiB = 0 B, allocated: 7.611 MiB - 7.611 MiB = 0 B, pages: 6 - 0 = 6, loss: 24.000 KiB = 100.000% },
    size_1000: { allocations: 3168 - 3168 = 0, requested: 3.011 MiB - 3.011 MiB = 0 B, allocated: 3.021 MiB - 3.021 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_1008: { allocations: 4813 - 4813 = 0, requested: 4.605 MiB - 4.605 MiB = 0 B, allocated: 4.627 MiB - 4.627 MiB = 0 B, pages: 4 - 0 = 4, loss: 16.000 KiB = 100.000% },
    size_1016: { allocations: 3214 - 3214 = 0, requested: 3.104 MiB - 3.104 MiB = 0 B, allocated: 3.114 MiB - 3.114 MiB = 0 B, pages: 3 - 0 = 3, loss: 12.000 KiB = 100.000% },
    size_1024: { allocations: 206601 - 206601 = 0, requested: 135.155 MiB - 135.155 MiB = 0 B, allocated: 201.759 MiB - 201.759 MiB = 0 B, pages: 87 - 0 = 87, loss: 348.000 KiB = 100.000% },
    size_2048: { allocations: 102871 - 102871 = 0, requested: 50.132 MiB - 50.132 MiB = 0 B, allocated: 200.920 MiB - 200.920 MiB = 0 B, pages: 90 - 0 = 90, loss: 360.000 KiB = 100.000% },
}
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 49.50s
```


### Ориентировочный объём работ этой части лабораторки

```console
 ku/src/allocator/dispatcher.rs | 80 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++--------
 ku/src/allocator/fixed_size.rs | 66 ++++++++++++++++++++++++++++++++++++++++++++++++++++++------------
 2 files changed, 126 insertions(+), 20 deletions(-)
```
