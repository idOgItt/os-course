## Отображение виртуальных страниц в физические фреймы

За это отображение отвечает структура
[`kernel::memory::mapping::Mapping`](../../doc/kernel/memory/mapping/struct.Mapping.html).
Фактически она реализует дерево большой арности --- 512, если закрыть глаза на возможность сослаться в узле на любой узел.
Такая возможность используется только в специфических случаях, например для реализации [рекурсивного отображения](https://wiki.osdev.org/Page_Tables#Recursive_mapping).

[`Mapping`](../../doc/kernel/memory/mapping/struct.Mapping.html) содержит уже немного знакомые нам поля:

- [`Mapping::page_table_root`](../../doc/kernel/memory/mapping/struct.Mapping.html#structfield.page_table_root) типа [`Frame`](../../doc/ku/memory/frage/type.Frame.html) --- физический фрейм с таблицей страниц самого верхнего уровня, корневой в дереве.
- [`Mapping::phys2virt`](../../doc/kernel/memory/mapping/struct.Mapping.html#structfield.phys2virt) типа [`Page`](../../doc/ku/memory/frage/type.Page.html) --- начало "окна", в которое отображена вся физическая память.

[`Mapping::phys2virt`](../../doc/kernel/memory/mapping/struct.Mapping.html#structfield.phys2virt)
нужен, чтобы с помощью функции
```rust
fn phys2virt_map(phys2virt: Page, address: Phys) -> Virt
```
по заданному физическому адресу, найти куда он отображён в "окне".
Это потребуется когда нам нужно будет записать в какую-нибудь
[`PageTable`](../../doc/ku/memory/mmu/type.PageTable.html).
Так как в
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html) таблица
[`PageTable`](../../doc/ku/memory/mmu/type.PageTable.html) следующего вниз уровня
задаётся именно физическим адресом.

> Подумайте, а можно ли было бы в
> [`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)
> хранить виртуальный адрес
> [`PageTable`](../../doc/ku/memory/mmu/type.PageTable.html),
> на которую ссылается данная
> [`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)?

Отображение виртуальных страниц на физические фреймы:

![](2-mm-6-address-space-2-virt-to-phys.svg)

Про отображение страниц можно почитать на сайте [osdev](https://wiki.osdev.org/), нас интересует 64-битный вариант для x86-64:

- [Memory management](https://wiki.osdev.org/Memory_management).
- [Paging](https://wiki.osdev.org/Paging).

Ещё более подробный и обстоятельный разбор есть в блоге [Writing an OS in Rust](https://os.phil-opp.com/):

- [Introduction to Paging](https://os.phil-opp.com/paging-introduction/), [перевод](https://habr.com/ru/post/436606/).
- [Paging Implementation](https://os.phil-opp.com/paging-implementation/), [перевод](https://habr.com/ru/post/445618/).


### Задача 3 --- [`Mapping`](../../doc/kernel/memory/mapping/struct.Mapping.html)


#### Отображение виртуальных страниц на физические фреймы

Страничное преобразование устроено как показано на схеме.
Стрелки ведут из физических адресов, хранящихся в регистре
[`CR3`](https://wiki.osdev.org/CPU_Registers_x86#CR3)
и в элементах
[`ku::memory::mmu::PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)
таблиц
[`ku::memory::mmu::PageTable`](../../doc/ku/memory/mmu/type.PageTable.html),
в целевые фреймы и целевой байт.
А пунктиром показано какая часть битового представления виртуального адреса
([`Virt`](../../doc/ku/memory/addr/type.Virt.html))
используется как индекс в одной из таблиц
[`PageTable`](../../doc/ku/memory/mmu/type.PageTable.html),
либо как смещение внутри целевого фрейма.
Каждая [`PageTable`](../../doc/ku/memory/mmu/type.PageTable.html) занимает ровно один фрейм физической памяти.

![](2-mm-6-address-space-2-translate.svg)

Вооружившись этими знаниями, реализуйте [метод](../../doc/kernel/memory/mapping/struct.Mapping.html#method.path)
```rust
fn Mapping::path(
    &mut self,
    virt: Virt,
) -> Path
```

в файле [`kernel/src/memory/mapping.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/memory/mapping.rs).
Он принимает на вход виртуальный адрес `virt`, который нужно транслировать.
И возвращает максимально длинный отображённый в память префикс пути в дереве отображения страниц
[`Path`](../../doc/kernel/memory/mapping/struct.Path.html),
соответствующий входному виртуальному адресу `virt`.
Элементы
[`Path::nodes`](../../doc/kernel/memory/mapping/struct.Path.html#structfield.nodes)
заполняются виртуальными адресами соответствующих
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html).

[`Mapping::path()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.path)
должен пройти от корневой L3 по физическим фреймам, которые возвращает метод
[`fn PageTableEntry::frame() -> Result<Frame>`](../../doc/ku/memory/mmu/struct.PageTableEntry.html#method.frame)
для существующих узлов дерева
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html).

Если какая-то из этих
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)
не отображена на физическую память или содержит флаг
[`PageTableFlags::HUGE_PAGE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.HUGE_PAGE),
на этом построение пути заканчивается.

Реализуйте [метод](../../doc/kernel/memory/mapping/struct.Path.html#method.map_intermediate)
```rust
pub fn Path::map_intermediate(
    &mut self,
    flags: PageTableFlags,
) -> Result<()> {
```

Он должен достроить путь в дереве отображения страниц,
соответствующий виртуальному адресу
[`Path::virt`](../../doc/kernel/memory/mapping/struct.Path.html#structfield.virt),
до листьевого уровня (L0 на схеме).
Чтобы вызывающая функция могла потом как-либо модифицировать отображение
[`Path::virt`](../../doc/kernel/memory/mapping/struct.Path.html#structfield.virt) ---
изменить его флаги, поменять физический адрес.

Недостающие узлы этот метод аллоцирует с помощью глобального
[`FRAME_ALLOCATOR`](../../doc/kernel/memory/struct.FRAME_ALLOCATOR.html).
Только что аллоцированную
[`PageTable`](../../doc/ku/memory/mmu/type.PageTable.html)
нужно обязательно очистить ---
все записи в ней должны быть равны значению, которое возвращает метод
[`PageTableEntry::default()`](../../doc/ku/memory/mmu/struct.PageTableEntry.html#method.default).
Только после этого можно установить этот фрейм в промежуточную
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)
методом
[`PageTableEntry::set_frame()`](../../doc/ku/memory/mmu/struct.PageTableEntry.html#method.set_frame).
Теперь промежуточная таблица отображена в память.

Если в промежуточной таблице встретилась запись, в которой
[`PageTableEntry::flags()`](../../doc/ku/memory/mmu/struct.PageTableEntry.html#method.flags)
содержит флаг
[`PageTableFlags::HUGE_PAGE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.HUGE_PAGE),
верните
[`Error::Unimplemented`](../../doc/kernel/error/enum.Error.html#variant.Unimplemented).
Такая запись не является промежуточной, а задаёт большую страницу, ---
[x86-64 поддерживает](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details) страницы
[размеров `4 KiB`, `2 MiB` и `1 GiB`](https://en.wikipedia.org/wiki/Page_(computer_memory)#Multiple_page_sizes).
Мы не будем их поддерживать.
Но они нам будут попадаться, потому что с их помощью
[bootloader](../../doc/bootloader/index.html)
отображает всю физическую память в "окно" в виртуальной.
Делается это для экономии физических фреймов на само отображение.

Кроме того, есть ещё такой момент.
Если вызывающая функция захочет отобразить
[`Path::virt`](../../doc/kernel/memory/mapping/struct.Path.html#structfield.virt),
например, с возможностью записи, и установит
соответствующий флаг
[`PageTableFlags::WRITABLE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.WRITABLE)
только в том элементе
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)
таблицы L0, которую вернёт
[`Path::get_mut()`](../../doc/kernel/memory/mapping/struct.Path.html#method.get_mut),
этого может оказаться недостаточно.
Дело в том, что процессор пересечёт флаги [`PageTableFlags::WRITABLE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.WRITABLE)
из всех промежуточных
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)
по схеме `И`.
И если в какой-то из промежуточных
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)
флаг
[`PageTableFlags::WRITABLE`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.WRITABLE)
будет сброшен, то запись по виртуальному адресу `virt` будет запрещена,
несмотря на то что в
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)
таблицы L0, она разрешена.
Поэтому во всех промежуточных, не только аллоцируемых,
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)
функция
[`Path::map_intermediate()`](../../doc/kernel/memory/mapping/struct.Path.html#method.map_intermediate)
должна будет включить флаги, заданные ей в аргументе `flags`.
И никакие флаги промежуточных
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html)
она не должна случайно сбросить при этой операции.
Альтернативой было бы либо сразу включать все доступы в промежуточных
[`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html).
Также заметьте, что включать нужно только флаги доступа, они задаются константой
[`ku::memory::mmu::FULL_ACCESS`](../../doc/ku/memory/mmu/constant.FULL_ACCESS.html).
То есть, включать нужно флаги которые есть в пересечении --- `flags & FULL_ACCESS`.

Выделение физического фрейма под отсутствующую промежуточную таблицу
[`PageTable`](../../doc/ku/memory/mmu/type.PageTable.html)
и его зануление
рекомендуется вынести во вспомогательную [функцию](../../doc/kernel/memory/mapping/struct.Mapping.html#method.allocate_node)

```rust
fn allocate_node(
    &mut self,
) -> Result<Frame>
```

в файле [`kernel/src/memory/mapping.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/memory/mapping.rs).

При реализации `Mapping::path()`, `Path::map_intermediate()` и `Mapping::allocate_node()` вам также могут пригодиться:

- Поле [`Mapping::page_table_root`](../../doc/kernel/memory/mapping/struct.Mapping.html#structfield.page_table_root), оно содержит адрес фрейма корневой таблицы L3, то есть фактически копию регистра [`CR3`](https://wiki.osdev.org/CPU_Registers_x86#CR3).
- Константа [`ku::memory::mmu::PAGE_TABLE_ROOT_LEVEL`](../../doc/ku/memory/mmu/constant.PAGE_TABLE_ROOT_LEVEL.html) --- та самая 3 из обозначения L3 для корневой таблицы страниц --- третий уровень считая с нуля.
- Константа [`ku::memory::mmu::PAGE_TABLE_LEAF_LEVEL`](../../doc/ku/memory/mmu/constant.PAGE_TABLE_LEAF_LEVEL.html) --- та самая 0 из обозначения L0 для листьевой таблицы страниц.
- Конструкция `unsafe { virt_addr.try_into_mut::<PageTable>()? }`, которая превращает [`Virt`](../../doc/ku/memory/addr/type.Virt.html) в `&mut PageTable`.
- Метод [`core::mem::MaybeUninit::zeroed()`](https://doc.rust-lang.org/nightly/core/mem/union.MaybeUninit.html#method.zeroed) для зануления аллоцированной памяти под выделяемую таблицу страниц. Нулевое значение подойдёт для [`PageTableEntry`](../../doc/ku/memory/mmu/struct.PageTableEntry.html), --- с точки зрения компилятора сгодится любое значение оборачиваемого ею типа [`usize`](https://doc.rust-lang.org/nightly/core/primitive.usize.html), а с точки зрения процессора, подойдёт любое значение, если [`PageTableFlags::PRESENT`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#associatedconstant.PRESENT) будет сброшен.
- Цикл, чтобы обойти уровни таблицы. Для уменьшения потенциальных возможностей совершить ошибку, можно было бы сделать таблицы страниц разных уровней не совместимыми в системе типов, [как здесь](https://os.phil-opp.com/page-tables/#some-clever-solution). Тогда компилятор не дал бы их перепутать в коде. Но такой подход, к сожалению, не даст написать обычный цикл по уровням, только рекурсию на обобщённых функциях или копипасту. Поэтому в Nikka выбор сделан в пользу одного и того же типа для узлов всех уровней.
- Итерирование в обратном порядке: `for i in (5..10).rev()`, или же `for i in (5..=10).rev()` если `10` должно быть включительно.

Теперь должена проходить первая часть теста

```console
$ (cd kernel; cargo test --test 2-mm-3-translate)
...
2_mm_3_translate::t0_path-----------------------------------
18:30:23 0 D present_virt = 0v100_0020_0FFC; path = L3: 0vFFFF_F000_0000_1010 -> L2: 0vFFFF_F000_0070_7000 -> L1: 0vFFFF_F000_0070_8008 -> L0: 0vFFFF_F000_0070_F000 => 0p27_DFFC; level = 0; pte = 637 @ 0p27_D000 0--DA---WPX(0x63)
18:30:23 0 D allocated unique user virtual address; virt = 0v1000_0000_0000
18:30:23 0 D non_present_virt = 0v1000_0000_0000; path = L3: 0vFFFF_F000_0000_1100 (non-present); level = 3; pte = <non-present>
18:30:23 0 D huge_virt = 0vFFFF_F000_0000_0000; path = L3: 0vFFFF_F000_0000_1F00 -> L2: 0vFFFF_F000_0071_0000 -> L1: 0vFFFF_F000_0071_1000 (huge); level = 1; pte = 0 @ 0p0 0-H-A---WPX(0xA3)
2_mm_3_translate::t0_path-------------------------- [passed]

2_mm_3_translate::t1_translate------------------------------
18:30:23 0 D pte = 638 @ 0p27_E000 0--DA---WPX(0x63)
18:30:23 0 D read_ptr = 0xfffff0000027e330; write_ptr = 0x10000201330
18:30:23 0 D write_value = 0; read_value = 0; variable = 0
18:30:23 0 D write_value = 1; read_value = 1; variable = 1
18:30:23 0 D write_value = 2; read_value = 2; variable = 2
18:30:23 0 D write_value = 3; read_value = 3; variable = 3
18:30:23 0 D write_value = 4; read_value = 4; variable = 4
2_mm_3_translate::t1_translate--------------------- [passed]

2_mm_3_translate::t2_map_intermediate-----------------------
18:30:23 0 D allocated unique user virtual address; virt = 0v1080_0000_0000
18:30:23 0 D pte = <non-present>
18:30:23 0 D allocated unique user virtual address; virt = 0v1100_0000_0000
2_mm_3_translate::t2_map_intermediate-------------- [passed]

2_mm_3_translate::t3_no_excessive_intermediate_flags--------
18:30:23 0 D allocated unique user virtual address; virt = 0v1180_0000_0000
18:30:23 0 D pte = ()
2_mm_3_translate::t3_no_excessive_intermediate_flags [passed]

2_mm_3_translate::t4_huge_page------------------------------
2_mm_3_translate::t4_huge_page--------------------- [passed]

2_mm_3_translate::t5_no_page--------------------------------
18:30:23 0 D allocated unique user virtual address; virt = 0v1200_0000_0000
2_mm_3_translate::t5_no_page----------------------- [passed]

2_mm_3_translate::t6_build_map------------------------------
18:30:23 0 D virtual to physical mapping
18:30:23 0 D block = [0v1000, 0v1_6000), size 84.000 KiB -> [0p1000, 0p1_6000), size 84.000 KiB, 0-------WPX(0x3)
18:30:23 0 D block = [0vA_0000, 0vC_0000), size 128.000 KiB -> [0pA_0000, 0pC_0000), size 128.000 KiB, 0-------WPX(0x3)
18:30:23 0 D block = [0v20_0000, 0v21_7000), size 92.000 KiB -> [0p40_0000, 0p41_7000), size 92.000 KiB, 0--------P-(0x8000000000000001)
18:30:23 0 D block = [0v21_7000, 0v32_9000), size 1.070 MiB -> [0p41_6000, 0p52_8000), size 1.070 MiB, 0--------PX(0x1)
18:30:23 0 D block = [0v32_9000, 0v34_3000), size 104.000 KiB -> [0p52_7000, 0p54_1000), size 104.000 KiB, 0-------WP-(0x8000000000000003)
18:30:23 0 D block = [0v34_3000, 0v34_4000), size 4.000 KiB -> [0p1_7000, 0p1_8000), size 4.000 KiB, 0-------WP-(0x8000000000000003)
18:30:23 0 D block = [0v34_4000, 0v44_7000), size 1.012 MiB -> [0p54_1000, 0p64_4000), size 1.012 MiB, 0-------WP-(0x8000000000000003)
18:30:23 0 D block = [0v44_7000, 0v44_D000), size 24.000 KiB -> [0p1_8000, 0p1_E000), size 24.000 KiB, 0-------WP-(0x8000000000000003)
18:30:23 0 D block = [0v100_0000_0000, 0v100_0000_1000), size 4.000 KiB -> [0p1_6000, 0p1_7000), size 4.000 KiB, 0-------WPX(0x3)
18:30:23 0 D block = [0v100_0000_2000, 0v100_0008_3000), size 516.000 KiB -> [0p1_E000, 0p9_F000), size 516.000 KiB, 0-------WPX(0x3)
18:30:24 0 D block = [0v100_0008_3000, 0v100_0020_2000), size 1.496 MiB -> [0p10_0000, 0p27_F000), size 1.496 MiB, 0-------WPX(0x3)
18:30:24 0 D block = [0vFFFF_F000_0000_0000, 0vFFFF_F001_0020_0000), size 4.002 GiB -> [0p0, 0p1_0020_0000), size 4.002 GiB, 0-H-----WPX(0x83)
18:30:24 0 D virtual address space
18:30:24 0 D block = [0v1000, 0v1_6000), size 84.000 KiB, 0-------WPX(0x3)
18:30:24 0 D block = [0vA_0000, 0vC_0000), size 128.000 KiB, 0-------WPX(0x3)
18:30:24 0 D block = [0v20_0000, 0v21_7000), size 92.000 KiB, 0--------P-(0x8000000000000001)
18:30:24 0 D block = [0v21_7000, 0v32_9000), size 1.070 MiB, 0--------PX(0x1)
18:30:24 0 D block = [0v32_9000, 0v44_D000), size 1.141 MiB, 0-------WP-(0x8000000000000003)
18:30:24 0 D block = [0v100_0000_0000, 0v100_0000_1000), size 4.000 KiB, 0-------WPX(0x3)
18:30:24 0 D block = [0v100_0000_2000, 0v100_0020_2000), size 2.000 MiB, 0-------WPX(0x3)
18:30:25.195 0 D block = [0vFFFF_F000_0000_0000, 0vFFFF_F001_0020_0000), size 4.002 GiB, 0-H-----WPX(0x83)
2_mm_3_translate::t6_build_map--------------------- [passed]

2_mm_3_translate::t7_duplicate_drop-------------------------
18:30:25.213 0 D allocated unique user virtual address; virt = 0v1280_0000_0000
18:30:25.219 0 D allocated unique user virtual address; virt = 0v1300_0000_0000
panicked at kernel/src/memory/mapping.rs:207:9:
not implemented
  ...
--------------------------------------------------- [failed]
```


#### Создание полной копии виртуального отображения

В будущем, при создании нового процесса, нам понадобится скопировать существующий
[`Mapping`](../../doc/kernel/memory/mapping/struct.Mapping.html)
в новый.
Это делает метод
[`Mapping::duplicate()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.duplicate):
```rust
fn duplicate(&self) -> Result<Self> {
    let mut result = Self::new(Frame::zero(), self.phys2virt);
    result.page_table_root = Self::duplicate_page_table(&mut result, self, self.page_table_root, PAGE_TABLE_ROOT_LEVEL)?;
    Ok(result)
}
```
Основную работу он перекладывает на рекурсивный метод
[`Mapping::duplicate_page_table()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.duplicate_page_table),
запуская корневой вызов рекурсии.

Реализуйте [метод](../../doc/kernel/memory/mapping/struct.Mapping.html#method.duplicate_page_table)
```rust
fn Mapping::duplicate_page_table(
    &self,
    dst: &mut Mapping,
    src_frame: Frame,
    level: u32,
) -> Result<Frame>
```

в файле [`kernel/src/memory/mapping.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/memory/mapping.rs)
сейчас, пока помните как устроено страничное отображение.
Он принимает на вход:

- Новое отображение `dst`, которое мы создаём как копию исходного.
- Исходное отображение `self`.
- Исходный узел `PageTable` таблицы страниц в виде содержащего его фрейма `src_frame`.
- Уровень этого узла `level` от [`PAGE_TABLE_ROOT_LEVEL = 3`](../../doc/ku/memory/mmu/constant.PAGE_TABLE_ROOT_LEVEL.html)
до [`PAGE_TABLE_LEAF_LEVEL = 0`](../../doc/ku/memory/mmu/constant.PAGE_TABLE_LEAF_LEVEL.html).

И должен создать копию заданного `src_frame` узла.
Возвращает он либо эту копию, либо возникшую в процессе работы ошибку.
Этот метод копирует только страничное отображение, то есть все `PageTable` уровней L3--L0,
но не фреймы, на которые указывает это отображение.
То есть, должно получиться отображение,
которое переводит те же виртуальные адреса в те же физические адреса.
Разделяя таким образом отображённую часть физической памяти,
но дублируя физическую память под само страничное отображение (`PageTable` уровней L3--L0).
Если бы `PageTable` разделяли ту же физическую память, то уже после того как копирование отработало,
модификации исходного `self` приводили бы к модификации `dst` и наоборот.
А это не то что нам нужно.
Это означает, что

- `PageTable` уровней L3--L1 включительно нужно пересоздать, заполняя их `PageTableEntry` результатами рекурсивных вызовов `Mapping::duplicate_page_table()`.
- А вот `PageTable` уровня L0 нужно просто скопировать как есть, их записи `PageTableEntry` должны вести в те же физические фреймы, что и в исходном отображении `self`. Но только если соответствующая запись ведёт на страницу, принадлежащую ядру. Ссылки уровня L0 на страницы, принадлежащие пользователю --- `PageTableFlags::USER_ACCESSIBLE`, --- копировать не нужно.
- На этот раз `PageTableFlags::HUGE_PAGE` нужно корректно обработать. Это означает, что рекурсивно спускаться в записи `PageTableEntry` для которых этот флаг включён не нужно. С этими записями нужно поступить как с записями на уровне L0, --- скопировать в точности из `self`.
- Не используемые записи в новых [`PageTable`](../../doc/ku/memory/mmu/type.PageTable.html) нужно почистить, как и в [`Mapping::map_intermediate()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.map_intermediate).

Для физических фреймов, на которые указывают скопированные записи листьевого уровня L0, нужно вызвать
[`FrameAllocator::reference()`](../../doc/kernel/memory/enum.FrameAllocator.html#method.reference).
Этот метод увеличивает число ссылок на физический фрейм.
И позволяет понять, что физический фрейм нельзя считать свободным пока оба отображения `self` и `dst` существуют.
А ведь именно указанные фреймы мы разделяем, используя сразу в обоих отображениях.
Для выделения новых физических фреймов используйте
[`FrameAllocator::allocate()`](../../doc/kernel/memory/enum.FrameAllocator.html#method.allocate).
В обоих случаях обращайтесь к глобальному
[`FRAME_ALLOCATOR`](../../doc/kernel/memory/struct.FRAME_ALLOCATOR.html).

Также вам могут пригодиться методы

- [`unsafe fn Mapping::page_table_ref(&self, frame: Frame) -> &PageTable`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.page_table_ref) и
- [`unsafe fn Mapping::page_table_mut(&mut self, frame: Frame) -> &mut PageTable`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.page_table_mut).

Они позволяют интерпретировать заданный физический фрейм `frame` как узел `PageTable` таблицы страниц.
И различаются только возможностью менять эту таблицу.

Учтите, что в рекурсивный вызов передаётся изменяемая ссылка `dst: &mut Mapping`.
А значит, в момент рекурсивного вызова не должно быть живых локальных переменных ссылающихся на `dst` прямо или косвенно.
Rust не даст скомпилировать такой код из-за нарушения [владения](https://doc.rust-lang.ru/book/ch04-00-understanding-ownership.html) `dst`, ---
только одна из функций, вызывающая или вызываемая, может эксклюзивно заимствовать `dst` в каждый момент времени.
А [изменяемая ссылка и означает эксклюзивность заимствования](https://doc.rust-lang.ru/book/ch04-02-references-and-borrowing.html#%D0%98%D0%B7%D0%BC%D0%B5%D0%BD%D1%8F%D0%B5%D0%BC%D1%8B%D0%B5-%D1%81%D1%81%D1%8B%D0%BB%D0%BE%D1%87%D0%BD%D1%8B%D0%B5-%D0%BF%D0%B5%D1%80%D0%B5%D0%BC%D0%B5%D0%BD%D0%BD%D1%8B%D0%B5).


#### Удаление виртуального отображения

Раз есть способ создать новое отображение, которое расходует физические фреймы, значит должен быть и способ удалить его, вернув эти фреймы в систему.
Это делает реализация типажа [`core::ops::Drop`](https://doc.rust-lang.org/nightly/core/ops/trait.Drop.html):
```rust
impl Drop for Mapping {
    fn drop(&mut self) {
        assert!(Self::current_page_table_root() != self.page_table_root);

        if self.is_valid() {
            self.drop_subtree(self.page_table_root, PAGE_TABLE_ROOT_LEVEL);
        }
    }
}
```
В строчке `assert!(Self::current_page_table_root() != self.page_table_root)` проверяется, что мы не пытаемся удалить отображение, которое в данный момент загружено в регистр
[`CR3`](https://wiki.osdev.org/CPU_Registers_x86#CR3)
и является активным виртуальным пространством.
Далее вся работа перекладывается на рекурсивный метод
[`Mapping::drop_subtree()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.drop_subtree).

Реализуйте [метод](../../doc/kernel/memory/mapping/struct.Mapping.html#method.drop_subtree)
```rust
fn Mapping::drop_subtree(
    &mut self,
    node: Frame,
    level: u32,
    drop_used: bool,
) -> bool
```
в файле [`kernel/src/memory/mapping.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/memory/mapping.rs).

Он удаляет узлы дерева отображения.

- Если `drop_used = true`, удаляются все узлы. Это как раз нужно в [`Mapping::drop()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.drop).
- Если `drop_used = false`, удаляются только узлы, через которые не отображена ни одна страница. То есть, узлы в которых все элементы равны [`PageTableEntry::default()`](../../doc/ku/memory/mmu/struct.PageTableEntry.html#method.default), либо становятся таковыми после удаления ненужных узлов следующих уровней. Это нужно для очистки не использующейся памяти методом [`Mapping::unmap_unused_intermediate()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.unmap_unused_intermediate) и корректной работы функции `forbid_frame_leaks()`, которая следит за отсутствием утечек физических фреймов в тестах. В этом режиме используется возвращаемое значение, которое говорит о том, был ли удалён текущий узел.

Для освобождения фреймов используйте
[`FrameAllocator::deallocate()`](../../doc/kernel/memory/enum.FrameAllocator.html#method.deallocate),
он уменьшит количество ссылок на фрейм и освободит его, когда ссылок не останется.
Перед освобождением фрейма, занятого под одну из `PageTable`, то есть из записей `PageTableEntry` уровней L3--L1,
нужно спуститься в этот фрейм, рекурсивно вызвав `Mapping::drop_subtree()`.
Пользовательские фреймы нужно освобождать так же как и фреймы, занятые под `PageTable`.
А вот с записями у которых включён флаг `PageTableFlags::HUGE_PAGE` делать ничего не нужно --- ни освобождать, ни спускаться в них рекурсивно.


### Проверьте себя

Запустите тест, теперь он должен проходить полностью:

```console
$ (cd kernel; cargo test --test 2-mm-3-translate)
...
2_mm_3_translate::t1_translate------------------------------
18:42:21 0 D pte = 638 @ 0p27_E000 0--DA---WPX(0x63)
18:42:21 0 D read_ptr = 0xfffff0000027e330; write_ptr = 0x10000201330
18:42:21 0 D write_value = 0; read_value = 0; variable = 0
18:42:21 0 D write_value = 1; read_value = 1; variable = 1
18:42:21 0 D write_value = 2; read_value = 2; variable = 2
18:42:21 0 D write_value = 3; read_value = 3; variable = 3
18:42:21 0 D write_value = 4; read_value = 4; variable = 4
2_mm_3_translate::t1_translate--------------------- [passed]

2_mm_3_translate::t2_map_intermediate-----------------------
18:42:21 0 D pte = <non-present>
2_mm_3_translate::t2_map_intermediate-------------- [passed]

2_mm_3_translate::t3_no_excessive_intermediate_flags--------
18:42:21 0 D pte = ()
2_mm_3_translate::t3_no_excessive_intermediate_flags [passed]

2_mm_3_translate::t4_huge_page------------------------------
2_mm_3_translate::t4_huge_page--------------------- [passed]

2_mm_3_translate::t5_no_page--------------------------------
2_mm_3_translate::t5_no_page----------------------- [passed]

2_mm_3_translate::t6_duplicate_drop-------------------------
18:42:22 0 I duplicate; address_space = "process" @ 0p7FD_6000
18:42:22 0 I drop; address_space = "process" @ 0p7FD_6000
18:42:22 0 I duplicate; address_space = "process" @ 0p7FC_3000
18:42:22 0 I drop; address_space = "process" @ 0p7FC_3000
2_mm_3_translate::t6_duplicate_drop---------------- [passed]

2_mm_3_translate::t7_drop_subtree---------------------------
18:42:23.067 0 D pte = <non-present>
18:42:23.209 0 D pte = ()
2_mm_3_translate::t7_drop_subtree------------------ [passed]

2_mm_3_translate::t9_no_frame-------------------------------
18:42:27.461 0 D real free frame count can be less than free frame count reported by boot frame allocator since it records reference and deallocation counts but does not do that actually; frame_count = 30942; real_frame_count = 30883
2_mm_3_translate::t9_no_frame---------------------- [passed]
18:42:27.477 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/memory/mapping.rs | 114 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++-------
 1 file changed, 105 insertions(+), 9 deletions(-)
```
