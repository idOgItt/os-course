## Проверки доступа процесса к памяти

В системных вызовах код пользователя будет передавать в ядро области памяти, описываемые `Block<Virt>`.
И код ядра должен будет что-то сделать с указанной памятью,
иногда читая или записывая в неё.

Ядро не может доверять коду пользователя. Поэтому перед чтением или записью в такие области памяти,
оно должно проверить, есть ли у пользователя соответствующий доступ.
За выполнение всех нужных проверок, не только указанного типа,
отвечают функции системных вызовов в файле [`kernel/src/process/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/syscall.rs), ---
это граница между кодом пользователя и кодом ядра.

А за фактическую реализацию проверок доступа к памяти отвечают методы в файле [`kernel/src/memory/address_space.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/memory/address_space.rs):

- [`kernel::memory::address_space::AddressSpace::check_permission<T>()`](../../doc/kernel/memory/address_space/struct.AddressSpace.html#method.check_permission) --- проверяет доступность памяти на чтение.
- [`kernel::memory::address_space::AddressSpace::check_permission_mut<T>()`](../../doc/kernel/memory/address_space/struct.AddressSpace.html#method.check_permission_mut) --- проверяет доступность памяти на запись.

Оба метода принимают на вход `Block<Virt>`, а возвращают либо ошибку доступа
[`Error::PermissionDenied`](../../doc/kernel/error/enum.Error.html#variant.PermissionDenied),
либо неизменяемый срез `&[T]` и изменяемый `&mut [T]` соответственно.
Для перевода `Block<Virt>` в срезы они пользуются методами
[`Block::<Virt>::try_into_slice()`](../../doc/ku/memory/block/struct.Block.html#method.try_into_slice) и
[`Block::<Virt>::try_into_mut_slice()`](../../doc/ku/memory/block/struct.Block.html#method.try_into_mut_slice).
Которые выполняют дополнительные проверки --- что `Block<Virt>` имеет допустимый адрес, а не
[`core::ptr::null()`](https://doc.rust-lang.org/nightly/core/ptr/fn.null.html),
что он выровнен подходящим для `T` образом и имеет подходящий для хранения `[T]` размер.

Основную же работу по проверке доступа к виртуальным адресам они перекладывают на вспомогательный метод

```rust
fn AddressSpace::check_permission_common(
    &mut self,
    block: &Block<Virt>,
    flags: PageTableFlags,
) -> Result<()>
```

Он должен пройтись по всем страницам заданного `block`, проверяя что они отображены в адресное пространство `self`
с флагами `PageTableFlags::PRESENT`, `PageTableFlags::USER_ACCESSIBLE` и заданными на вход `flags` одновременно.


### Задача 2 --- проверка доступа к памяти

Реализуйте методы [`AddressSpace::check_permission_common()`](../../doc/kernel/memory/address_space/struct.AddressSpace.html#method.check_permission_common), [`AddressSpace::check_permission<T>()`](../../doc/kernel/memory/address_space/struct.AddressSpace.html#method.check_permission) и [`AddressSpace::check_permission_mut<T>()`](../../doc/kernel/memory/address_space/struct.AddressSpace.html#method.check_permission_mut) в файле [`kernel/src/memory/address_space.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/memory/address_space.rs).

Вам могут пригодиться

- Метод [`fn Block::<Virt>::enclosing() -> Block<Page>`](../../doc/ku/memory/block/struct.Block.html#method.enclosing), который для заданного блока виртуальных адресов возвращает минимальный содержащий его блок страниц виртуальной памяти.
- Метод [`Mapping::translate()`](../../doc/kernel/memory/mapping/struct.Mapping.html#method.translate).
- Метод [`PageTableFlags::contains()`](../../doc/ku/memory/mmu/struct.PageTableFlags.html#method.contains).
- [Итерирование](../../doc/ku/memory/block/struct.Block.html#impl-IntoIterator-for-Block%3CT%3E) по страницам блока с помощью вызова [`Block::<Page>::into_iter()`](../../doc/ku/memory/block/struct.Block.html#method.into_iter).


### Проверьте себя

Запустите тест `4-process-2-permission-checks` из файлa
[`kernel/tests/4-process-2-permission-checks.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-2-permission-checks.rs):

```console
$ (cd kernel; cargo test --test 4-process-2-permission-checks)
...
4_process_2_permission_checks::non_present------------------
20:36:40 0 D pages = [0v7FFF_FFE7_4000, 0v7FFF_FFE7_8000), size 16.000 KiB
4_process_2_permission_checks::non_present--------- [passed]

4_process_2_permission_checks::stress-----------------------
20:36:41 0 D pages = [0v7FFF_FFE7_3000, 0v7FFF_FFE7_4000), size 4.000 KiB
20:36:41 0 D pages = [0v7FFF_FFE7_1000, 0v7FFF_FFE7_3000), size 8.000 KiB
20:36:42.659 0 D pages = [0v7FFF_FFE6_E000, 0v7FFF_FFE7_1000), size 12.000 KiB
20:36:49.731 0 D pages = [0v7FFF_FFE6_A000, 0v7FFF_FFE6_E000), size 16.000 KiB
4_process_2_permission_checks::stress-------------- [passed]

4_process_2_permission_checks::user_rw----------------------
20:37:14.179 0 D pages = [0v7FFF_FFE6_6000, 0v7FFF_FFE6_A000), size 16.000 KiB
4_process_2_permission_checks::user_rw------------- [passed]
20:37:14.195 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/memory/address_space.rs | 30 ++++++++++++++++++++++++------
 1 file changed, 24 insertions(+), 6 deletions(-)
```
