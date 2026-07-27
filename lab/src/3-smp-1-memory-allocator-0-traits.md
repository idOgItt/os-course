## Аллокатор памяти общего назначения

В этой части лабораторной работы вам нужно будет провязать
Rust'овский интерфейс аллокатора памяти общего назначения с реализованными во
[второй лабораторной работе](../../lab/book/2-mm-0-intro.html)
методами адресного пространства
[`kernel::memory::address_space::AddressSpace`](../../doc/kernel/memory/address_space/struct.AddressSpace.html).

После этого станет доступна
[`alloc`](https://doc.rust-lang.org/nightly/alloc/index.html)
часть стандартной библиотеки.
И в последующих задачах лабораторок мы сможем использовать стандартные контейнеры, такие как
[`alloc::vec::Vec`](https://doc.rust-lang.org/nightly/alloc/vec/struct.Vec.html)
и
[`alloc::collections::vec_deque::VecDeque`](https://doc.rust-lang.org/nightly/alloc/collections/vec_deque/struct.VecDeque.html).
Что гораздо удобнее, чем методы семейства
[`AddressSpace::map_slice()`](../../doc/kernel/memory/address_space/struct.AddressSpace.html#method.map_slice).


### Типажи [`core::alloc::Allocator`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html) и [`ku::allocator::dry::DryAllocator`](../../doc/ku/allocator/dry/trait.DryAllocator.html)

Прежде всего, изучите Rust'овский интерфейс аллокатора памяти общего назначения --- типаж
[`core::alloc::Allocator`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html).
Убедитесь, что вы понимаете:

- что такое [`core::alloc::Layout`](https://doc.rust-lang.org/nightly/core/alloc/struct.Layout.html);
- что такое [`core::ptr::NonNull`](https://doc.rust-lang.org/nightly/core/ptr/struct.NonNull.html);
- чем отличаются `NonNull<u8>` и `NonNull<[u8]>`;
- что означают маркеры `unsafe` на самом типаже и на его методах.

Также изучите типаж
[`ku::allocator::dry::DryAllocator`](../../doc/ku/allocator/dry/trait.DryAllocator.html).
Он очень похож на
[`core::alloc::Allocator`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html),
и отличается только тем, что:

- Вместо двух методов [`core::alloc::Allocator::allocate()`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html#tymethod.allocate) и [`core::alloc::Allocator::allocate_zeroed()`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html#method.allocate_zeroed) имеет только один метод [`ku::allocator::dry::DryAllocator::dry_allocate()`](../../doc/ku/allocator/dry/trait.DryAllocator.html#tymethod.dry_allocate), принимающий признак необходимости занулить память [`ku::allocator::dry::Initialize`](../../doc/ku/allocator/dry/enum.Initialize.html). Это сделано, чтобы не дублировать код.
- Аналогично, вместо двух методов [`core::alloc::Allocator::grow()`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html#method.grow) и [`core::alloc::Allocator::grow_zeroed()`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html#method.grow_zeroed) он имеет только один метод [`ku::allocator::dry::DryAllocator::dry_grow()`](../../doc/ku/allocator/dry/trait.DryAllocator.html#tymethod.dry_grow).


### Типаж [`core::alloc::GlobalAlloc`](https://doc.rust-lang.org/nightly/core/alloc/trait.GlobalAlloc.html)

Типаж глобального аллокатора
[`core::alloc::GlobalAlloc`](https://doc.rust-lang.org/nightly/core/alloc/trait.GlobalAlloc.html)
немного отличается от
[`core::alloc::Allocator`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html).
Изучите и его, он понадобится для общего понимания и в
[не обязательной задаче](../../lab/book/3-smp-1-memory-allocator-2-small.html#%D0%97%D0%B0%D0%B4%D0%B0%D1%87%D0%B0-2--%D0%B0%D0%BB%D0%BB%D0%BE%D0%BA%D0%B0%D1%82%D0%BE%D1%80-%D0%BC%D0%B0%D0%BB%D0%B5%D0%BD%D1%8C%D0%BA%D0%B8%D1%85-%D0%B1%D0%BB%D0%BE%D0%BA%D0%BE%D0%B2-%D0%BF%D0%B0%D0%BC%D1%8F%D1%82%D0%B8)
про аллокатор маленьких блоков памяти.


### Типаж [`ku::allocator::big::BigAllocator`](../../doc/ku/allocator/big/trait.BigAllocator.html)

Мы не можем воспользоваться реализацией по умолчанию для методов
[`Allocator::grow()`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html#method.grow),
[`Allocator::grow_zeroed()`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html#method.grow_zeroed) и
[`Allocator::shrink()`](https://doc.rust-lang.org/nightly/core/alloc/trait.Allocator.html#method.shrink).
Нам придётся переопределять их поведение, потому что мы хотим сделать следующий трюк.
При переаллокации мы не будем копировать содержимое памяти.
А вместо этого поменяем отображение физических фреймов старого блока памяти, на место нового блока памяти.

Естественно, это пройдёт только для блоков памяти, которые выровнены на границы страниц.
Интерфейс аллокации таких блоков памяти задаётся типажом
[`ku::allocator::big::BigAllocator`](../../doc/ku/allocator/big/trait.BigAllocator.html).


### Статистика аллокатора [`ku::allocator::info::Info`](../../doc/ku/allocator/info/struct.Info.html)

Для отладки аллокации памяти в коде присутствует сбор статистики про выполняемые аллокатором операции.
Эта статистика доступна в виде структуры [`ku::allocator::info::Info`](../../doc/ku/allocator/info/struct.Info.html).
Её поля означают следующее:
- [`Info::allocations`](../../doc/ku/allocator/info/struct.Info.html#structfield.allocations) --- количество запросов к аллокатору.
- [`Info::requested`](../../doc/ku/allocator/info/struct.Info.html#structfield.requested) --- сколько памяти в байтах было запрошено у аллокатора.
- [`Info::allocated`](../../doc/ku/allocator/info/struct.Info.html#structfield.allocated) --- сколько памяти в байтах было выделено аллокатором, включая потери на округления размеров аллокаций.
- [`Info::pages`](../../doc/ku/allocator/info/struct.Info.html#structfield.pages) --- количество виртуальных страниц, которые были выделены для удовлетворения запросов.

Каждый такой счётчик является структурой
[`ku::allocator::info::Counter`](../../doc/ku/allocator/info/struct.Counter.html),
содержащей положительную и отрицательную компоненту:

- [`Counter::positive`](../../doc/ku/allocator/info/struct.Counter.html#structfield.positive) --- суммарный размер соответствующего параметра для аллокаций.
- [`Counter::negative`](../../doc/ku/allocator/info/struct.Counter.html#structfield.negative) --- суммарный размер соответствующего параметра для деаллокаций.

Метод [`Counter::balance()`](../../doc/ku/allocator/info/struct.Counter.html#method.balance) возвращает разность
`Counter::positive - Counter::negative`.
То есть, например:

- `info.allocated.positive()` --- количество выделенных байт за всё время работы, включая освобождённые в дальнейшем.
- `info.pages.negative()` --- количество освобождённых при деаллокациях страниц.
- `info.requested.balance()` --- количество байт, которые были запрошены у аллокатора, но ещё не освобождены.

От этих чисел хочется интерпретируемости и соответствия естественным инвариантам.
Если количество аллокаций больше чем деаллокаций, то естественно ожидать, что какая-то память ещё занята.
А в ситуации, когда ещё и на входе и выходе балансы должны быть нулевые, это означает утечку.
Если же деаллокаций больше чем аллокаций в ситуации когда и на входе и выходе балансы должны быть нулевые,
это наталкивает на мысли о наличии какой-то серьёзной ошибки.
В частности, реаллокация учитывается как одна деаллокация полного старого блока плюс одна аллокация полного
нового блока.
Не создавайте несколько деаллокаций по кусочку старого блока при вызовах `dry_shrink()` или
фиктивных аллокаций нулевого размера.
Это сломает естественные инварианты и интерпретируемость значений
[`ku::allocator::info::Info`](../../doc/ku/allocator/info/struct.Info.html),
а также не пройдёт тесты.

Поэтому обратите внимание, как в
[`ku::allocator::info::Info`](../../doc/ku/allocator/info/struct.Info.html)
отслеживаются аллокации и деаллокации, которые выполняет ваш код.

В логах эти счётчики будут печататься так:

```console
20:26:37 0 D info = { allocations: 9 - 8 = 1, requested: 284.000 KiB - 156.000 KiB = 128.000 KiB, allocated: 284.000 KiB - 156.000 KiB = 128.000 KiB, pages: 71 - 39 = 32, loss: 0 B = 0.000% }
```

Это означает, что всего было выполнено `9` аллокаций и `8` деаллокаций.
В аллокациях было суммарно запрошено `284.000 KiB`, а в деаллокациях суммарно было освобождено `156.000 KiB`,
что даёт текущее запрошенное потребление `128.000 KiB` в одной оставшейся на данный момент аллокации.
Так как аллокатор в данном примере постраничный, эти величины совпадают с количеством реально выделенной памяти,
а потери на фрагментацию нулевые.
