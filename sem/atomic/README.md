# atomic

В этой задаче вам нужно будет реализовать библиотеку для работы с атомарными операциями.
В `atomic/src/lib.rs` определены следующие функции:

```
fn atomic_load(atomic: *const usize) -> usize;
```
Атомарно читает по указателю `atomic`.

```
fn atomic_store(atomic: *mut usize, value: usize);
```
Атомарно записывает `value` по указателю `atomic`.

```
fn atomic_add(atomic: *mut usize, value: usize);
```
Атомарно прибавляет число `value` к значению по указателю `atomic`.
Для того чтобы выполнить сложение атомарно, используйте инструкцию `add` с префиксом `lock`.

```
fn atomic_sub(atomic: *mut usize, value: usize);
```
Атомарно вычитает число `value` из значения по указателю `atomic`.
Для того чтобы выполнить вычитание атомарно, используйте инструкцию `sub` с префиксом `lock`.

```
fn atomic_xchg(atomic: *mut usize, value: usize) -> usize;
```
Атомарно заменяет значение по указателю `atomic` на `value` и возвращает старое значение.
Используйте инструкцию `xchg`.
Напомним, для неё префикс `lock` указывать необязательно --- эта инструкция блокирует шину всегда.

```
fn atomic_cmpxchg(atomic: *mut usize, old_value: *mut usize, new_value: usize) -> bool;
```
Атомарно выполняет следующее действие.
Сравнивает значение по указателю `atomic` с `old_value`.
Если они совпадают, заменяет `atomic` на `new_value` и возвращает `true`.
Если они отличаются, записывает значение `atomic` в `old_value` и возвращает `false`.
Используйте инструкцию `cmpxchg` с префиксом `lock`.
Вам придётся прочитать значение флага `ZF`.
Для этого стоит воспользоваться инструкцией условной установки --- `setz`.
В списке инструкций она и другие такие инструкции объединены в одну `SETcc`.

Вашу реализацию запишите в `atomic/src/impl.s`.
При этом следуйте [x86-64 System V ABI](https://wiki.osdev.org/System_V_ABI#x86-64), ---
все эти функции помечены как `extern "C"`.
Считайте, что упорядочение памяти всюду должно быть `SeqCst`.

Если вы в затруднении, [поэкспериментируйте в godbolt](https://rust.godbolt.org/z/7oMocrzab) и почитайте
[документацию на инструкции](https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/40332.pdf).
