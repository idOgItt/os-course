## Состояние каждого процессора

В Nikka состояние отдельного процессора описано в структуре
[`kernel::smp::cpu::Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html),
определённой в [`kernel/src/smp/cpu.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/smp/cpu.rs).
Номер процессора, на котором сейчас происходит выполнение кода, продублирован в этой структуре и его можно узнать с помощью метода
[`Cpu::id()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.id).
Саму структуру
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)
текущего процессора можно получить из статического метода
[`Cpu::get()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.get).

Состояние каждого процессора включает:

- Стек ядра [`Cpu::kernel_stack`](../../doc/kernel/smp/cpu/struct.Cpu.html#structfield.kernel_stack). Процессоры обрабатывают системные вызовы и прерывания независимо, так что у каждого должен быть свой стек для этого.
- Также полезен дополнительный стек [`Cpu::page_fault_stack`](../../doc/kernel/smp/cpu/struct.Cpu.html#structfield.page_fault_stack), который используется во время обработки Page Fault. Он позволяет напечатать диагностику возникшего исключения даже в случае, когда оно вызвано исчерпанием основного стека ядра.
- [Task State Segment](https://en.wikipedia.org/wiki/Task_state_segment) [`Cpu::tss`](../../doc/kernel/smp/cpu/struct.Cpu.html#structfield.tss). В TSS хранится `RSP` ядра для текущего процессора. Разумеется, раз стеки разные, то и TSS тоже должны быть разные.
- Исполняющийся на данном CPU в текущий момент пользовательский процесс [`Cpu::current_process`](../../doc/kernel/smp/cpu/struct.Cpu.html#structfield.current_process). Пользовательские процессы выполняются независимо на разных ядрах. Следовательно, у каждого ядра текущий процесс должен быть своим.
- Системные регистры. У каждого процессора свои системные регистры. Так что функции их инициализации должны быть вызваны для каждого CPU. Явно в структуре [`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html) регистры не хранятся.
  - [`GDTR`](https://wiki.osdev.org/GDT#GDTR) --- Global Descriptor Table Register, содержит адрес [kernel::memory::gdt::Gdt](../../doc/kernel/memory/gdt/type.Gdt.html).
  - [`IDTR`](https://wiki.osdev.org/IDT#IDTR) --- Interrupt Descriptor Table Register, содержит адрес [x86_64::structures::idt::InterruptDescriptorTable](../../doc/x86_64/structures/idt/struct.InterruptDescriptorTable.html).
  - [`CR3`](https://wiki.osdev.org/CPU_Registers_x86-64#CR3) --- содержит адрес корневой таблицы страниц [ku::memory::mmu::PageTable](../../doc/ku/memory/mmu/type.PageTable.html).
  - [`TR`](https://wiki.osdev.org/CPU_Registers_x86-64#TR) --- Task Register, содержит адрес [x86_64::structures::tss::TaskStateSegment](../../doc/x86_64/structures/tss/struct.TaskStateSegment.html).
  - [`FS`](https://wiki.osdev.org/CPU_Registers_x86-64#FS.base.2C_GS.base) --- доступен для программных нужд.


### Задача 4 --- вектор структур [`kernel::smp::cpu::Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)


#### Инициализация TSS и регистра `TR`

Реализуйте [метод](../../doc/kernel/smp/cpu/struct.Cpu.html#method.set_tss)

```rust
fn Cpu::set_tss()
```
в файле
[`kernel/src/smp/cpu.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/smp/cpu.rs).

Он должен завести в GDT запись для Task State Segment текущего процессора.
И загрузить её в [`TR`](https://wiki.osdev.org/CPU_Registers_x86-64#TR) своего CPU.
Сам Task State Segment в поле [`Cpu::tss`](../../doc/kernel/smp/cpu/struct.Cpu.html#structfield.tss)
уже инициализирован методом
[`Cpu::new()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.new).

Для определения идентификатора текущего процессора можно использовать
[`Cpu::id()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.id).
Полезно проверить, что он выдаёт ровно такое же значение, что и метод
[`LocalApic::id()`](../../doc/kernel/smp/local_apic/struct.LocalApic.html#method.id).
Если это не так, значит инициализация local APIC ранее или запуск AP в одной из дальнейших задач выполняется неверно.

Также вам пригодятся:

- [`kernel::memory::gdt::GDT`](../../doc/kernel/memory/gdt/struct.GDT.html),
- [`kernel::memory::gdt::Gdt::set_tss()`](../../doc/kernel/memory/gdt/struct.SmpGdt.html#method.set_tss),
- [`kernel::memory::gdt::Gdt::tss()`](../../doc/kernel/memory/gdt/struct.SmpGdt.html#method.tss),
- [`x86_64::instructions::tables::load_tss`](../../doc/x86_64/instructions/tables/fn.load_tss.html).


#### Инициализация регистра `FS`

Регистр `FS` будем использовать для аналога
[thread--local storage](https://en.wikipedia.org/wiki/Thread-local_storage), ---
CPU--local storage.
Например, в момент получения системного вызова процессор работает на стеке пользователя.
Доверять стеку пользователя нельзя, поэтому до использования стека процессор должен переключиться на стек ядра.
Адрес стека ядра, предназначенного данному CPU, он возьмёт из нашего CPU--local storage, обратившись к нему через свой регистр `FS`.
Как вы уже догадались, сам CPU--local storage в Nikka --- это структура
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html).

Реализуйте [метод](../../doc/kernel/smp/cpu/struct.Cpu.html#method.set_fs)

```rust
fn Cpu::set_fs()
```

Он должен сохранить адрес структуры
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)
для текущего CPU в его собственном регистре `FS`.
Этот же адрес нужно будет записать в поле
[`Cpu::this`](../../doc/kernel/smp/cpu/struct.Cpu.html#structfield.this).
Дело в том, что адресовать память относительно регистра `FS` можно,
но узнать его линейный адрес можно только прочитав его напрямую
инструкцией [rdmsr](https://www.felixcloutier.com/x86/rdmsr).
Получить линейный адрес через инструкцию
[`lea`](https://www.felixcloutier.com/x86/lea)
не получится.
А адресовать что-либо через `FS` неудобно --- это можно сделать только в ассемблере.
Самым удобным представляется такой вариант.
Прочитать адрес структуры
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)
из поля
[`Cpu::this`](../../doc/kernel/smp/cpu/struct.Cpu.html#structfield.this),
обращаясь к самому этому полю относительно регистра `FS`,
то есть используя логический адрес `<сегментный_регистр>:<смещение>`.
Затем, имея линейный адрес структуры
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html),
получить её саму.
И дальше обращаться к её полям и методам без использования `FS`.
Что уже можно делать в Rust без помощи ассемблера.

По умолчанию, код пользователя не может менять регистры `FS` и `GS`. Так это и оставим.
Если бы мы разрешили ему их менять, например для реализации
[thread-local storage](https://en.wikipedia.org/wiki/Thread-local_storage) (TLS),
нам также понадобилась бы инструкция
[swapgs](https://www.felixcloutier.com/x86/swapgs)
при переключении между режимами ядра и пользователя.

Для сохранения значения в регистр `FS` вам пригодится метод
[`x86_64::registers::model_specific::FsBase::write()`](../../doc/x86_64/registers/model_specific/struct.FsBase.html#method.write).
Он принимает
[`x86_64::addr::VirtAddr`](../../doc/x86_64/addr/struct.VirtAddr.html),
а не
[`kernel::memory::Virt`](../../doc/kernel/memory/type.Virt.html).
Для преобразования
[`Virt`](../../doc/kernel/memory/type.Virt.html) в
[`VirtAddr`](../../doc/x86_64/addr/struct.VirtAddr.html)
можно использовать метод
[`Virt::into()`](../../doc/kernel/memory/addr/struct.Addr.html#method.into),
см. [примеры](../../doc/kernel/memory/type.Virt.html#%D0%9F%D1%80%D0%B5%D0%BE%D0%B1%D1%80%D0%B0%D0%B7%D0%BE%D0%B2%D0%B0%D0%BD%D0%B8%D1%8F-%D0%BC%D0%B5%D0%B6%D0%B4%D1%83-virt-%D0%B8-virtaddr).

Также вам может пригодиться метод
[`Virt::from_ref()`](../../doc/kernel/memory/addr/struct.Addr.html#method.from_ref).


#### Использование регистра `FS` для получения текущего [`kernel::smp::cpu::Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)

Реализуйте [метод](../../doc/kernel/smp/cpu/struct.Cpu.html#method.get)

```rust
unsafe fn Cpu::get() -> &'static mut Cpu
```

`unsafe` означает, что вызывающая сторона должна гарантировать, что вызывает
[`Cpu::get()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.get)
только после инициализации `FS` методом
[`Cpu::set_fs()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.set_fs).
Иначе в лучшем случае будет паника, а в худшем --- обращение к невалидной или занятой другими данными памяти.

Загрузите из инициализированного в предыдущей задаче поля
[`Cpu::this`](../../doc/kernel/smp/cpu/struct.Cpu.html#structfield.this)
адрес структуры
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html).
Однако
[`Cpu::this`](../../doc/kernel/smp/cpu/struct.Cpu.html#structfield.this) ---
это поле структуры, адрес которой мы как раз хотим узнать.
Поэтому нам придётся использовать обращение через инициализированный в предыдущей задаче регистр `FS`.

Тут придётся использовать ассемблер, например прибегнуть к макросу
[`asm!()`](https://doc.rust-lang.org/core/arch/macro.asm.html).
Документацию на который можно посмотреть в
[Rust By Example](https://doc.rust-lang.org/nightly/rust-by-example/unsafe/asm.html) и
[The Rust Reference](https://doc.rust-lang.org/nightly/reference/inline-assembly.html).
Наиболее полные руководства по архитектуре:
[AMD64 Architecture Programmer’s Manual](https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/40332.pdf) и
[Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://software.intel.com/en-us/download/intel-64-and-ia-32-architectures-sdm-combined-volumes-1-2a-2b-2c-2d-3a-3b-3c-3d-and-4).
Удобный справочник по командам ассемблера: [x86 and amd64 instruction reference](https://www.felixcloutier.com/x86/).

Можно посмотреть на ассемблерный код, который генерирует Rust для Nikka,
например:

```x86asm
$ (cd ku && build --release)
$ find target/ -name ku\*.s
target/kernel/release/deps/ku-a5b708644b0b7bda.s
$ rustfilt < target/kernel/release/deps/ku-a5b708644b0b7bda.s | grep -A20 'globl.*ku::memory::addr::Addr<ku::memory::addr::VirtTag>::is_high_area'
	.globl	ku::memory::addr::Addr<ku::memory::addr::VirtTag>::is_high_area
	.p2align	4, 0x90
	.type	ku::memory::addr::Addr<ku::memory::addr::VirtTag>::is_high_area,@function
ku::memory::addr::Addr<ku::memory::addr::VirtTag>::is_high_area:
.Lfunc_begin191:
	.loc	26 427 0
	.cfi_startproc
	push	rbp
	.cfi_def_cfa_offset 16
	.cfi_offset rbp, -16
	mov	rbp, rsp
	.cfi_def_cfa_register rbp
.Ltmp3026:
	.loc	26 428 9 prologue_end
	cmp	word ptr [rdi + 6], 0
	setne	al
	.loc	26 429 6 epilogue_begin
	pop	rbp
	.cfi_def_cfa rsp, 8
	ret
.Ltmp3027:
```

Или поставив дополнительную команду `asm` для `cargo`:
```x86asm
$ cargo install cargo-show-asm
$ cargo asm --package ku --lib --intel "ku::memory::mmu::read_cr3" | head -n20
    Finished release [optimized] target(s) in 0.08s

.section .text.ku::memory::mmu::read_cr3,"ax",@progbits
	.globl	ku::memory::mmu::read_cr3
	.p2align	4, 0x90
	.type	ku::memory::mmu::read_cr3,@function
ku::memory::mmu::read_cr3:

	.cfi_startproc
	push rbx
	.cfi_def_cfa_offset 16
	sub rsp, 304
	.cfi_def_cfa_offset 320
	.cfi_offset rbx, -16
	#APP

	mov rbx, cr3

	#NO_APP

	movabs rax, -9223372036854771713
```

На сайте
[rust.godbolt.org](https://rust.godbolt.org/z/1PsT7jjnx)
также можно посмотреть на ассемблерный код, который генерирует Rust.

Также понадобится передать в ассемблерную инструкцию константу, определяющую смещение поля
[`Cpu::this`](../../doc/kernel/smp/cpu/struct.Cpu.html#structfield.this)
внутри структуры
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html).
Для этого пригодится макрос
[`memoffset::offset_of!()`](../../doc/memoffset/macro.offset_of.html).
А
[передача констант в параметрах макроса `asm!()`](https://doc.rust-lang.org/unstable-book/language-features/asm-const.html)
доступна нам за счёт `#![feature(asm_const)]`:

```rust
asm!(
    "...",
    the_answer_to_the_ultimate_question_of_life_the_universe_and_everything = const 42,
);
```

Полученный адрес нужно будет сохранить в переменной подходящего типа,
поддерживаемого макросом `asm!()`, например `usize`.
Преобразовать его в ссылку на структуру
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)
можно с помощью
[`Virt`](../../doc/kernel/memory/type.Virt.html).
Он выполнит необходимые проверки на валидность этого адреса.
В случае, если проверки не пройдут и вернётся ошибка, можно паниковать.


#### Инициализация вектора структур [`kernel::smp::cpu::Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)

Чтобы пользоваться структурами
[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html),
для них и для содержащихся в них стеках нужно выделить память.

Реализуйте [функцию](../../doc/kernel/smp/cpu/fn.init_cpu_vec.html)

```rust
fn init_cpu_vec(cpu_count: usize) -> Result<Vec<Cpu>>
```

Все нужные стеки в количестве
`cpu_count *`[`Cpu::STACKS_PER_CPU`](../../doc/kernel/smp/cpu/struct.Cpu.html#associatedconstant.STACKS_PER_CPU)
можно выделить одним вызовом метода
[`kernel::memory::stack::Stack::new_slice()`](../../doc/kernel/memory/stack/struct.Stack.html#method.new_slice).
А для избежания переаллокаций при выделении памяти под
[`Vec`](https://doc.rust-lang.org/nightly/alloc/vec/struct.Vec.html)`<`[`Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)`>`
стоит использовать метод
[`alloc::vec::Vec::with_capacity()`](https://doc.rust-lang.org/nightly/alloc/vec/struct.Vec.html#method.with_capacity).

Далее нужно будет заполнить этот вектор передавая в метод
[`Cpu::new()`](../../doc/kernel/smp/cpu/struct.Cpu.html#method.new)
индекс соответствующей структуры в векторе в качестве идентификатора CPU
[`kernel::smp::local_apic::CpuId`](../../doc/kernel/smp/local_apic/type.CpuId.html)
и срез из части ранее выделенных стеков в количестве
[`Cpu::STACKS_PER_CPU`](../../doc/kernel/smp/cpu/struct.Cpu.html#associatedconstant.STACKS_PER_CPU)
штук.


### Проверьте себя

Запустите тест `3-smp-4-cpus` из файла
[`kernel/tests/3-smp-4-cpus.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/3-smp-4-cpus.rs):

```console
$ (cd kernel; cargo test --test 3-smp-4-cpus)
...
3_smp_4_cpus::initialized-----------------------------------
17:35:43 0 I cpu = 0; local_apic_id = 0
3_smp_4_cpus::initialized-------------------------- [passed]

3_smp_4_cpus::kernel_stacks---------------------------------
17:35:43 0 I cpu = 0; stack_guard = [0v7FFF_FFE7_C000, 0v7FFF_FFE7_D000), size 4.000 KiB; stack = [0v7FFF_FFE7_D000, 0v7FFF_FFE9_C000), size 124.000 KiB
17:35:43 0 I cpu = 0; stack_page = 34359737981 @ 0v7FFF_FFE7_D000; stack_frame = Frame(32600 @ 0p7F5_8000); stack_flags = PageTableFlags(PRESENT | WRITABLE | ACCESSED | DIRTY)
17:35:43 0 I cpu = 0; stack_guard_page = 34359737980 @ 0v7FFF_FFE7_C000; stack_guard_frame = NoFrame; stack_guard_flags = PageTableFlags(0x0)
17:35:43 0 I cpu = 1; stack_guard = [0v7FFF_FFEB_C000, 0v7FFF_FFEB_D000), size 4.000 KiB; stack = [0v7FFF_FFEB_D000, 0v7FFF_FFED_C000), size 124.000 KiB
17:35:43 0 I cpu = 1; stack_page = 34359738045 @ 0v7FFF_FFEB_D000; stack_frame = Frame(32536 @ 0p7F1_8000); stack_flags = PageTableFlags(PRESENT | WRITABLE | ACCESSED | DIRTY)
17:35:43 0 I cpu = 1; stack_guard_page = 34359738044 @ 0v7FFF_FFEB_C000; stack_guard_frame = NoFrame; stack_guard_flags = PageTableFlags(0x0)
17:35:43 0 I cpu = 2; stack_guard = [0v7FFF_FFEF_C000, 0v7FFF_FFEF_D000), size 4.000 KiB; stack = [0v7FFF_FFEF_D000, 0v7FFF_FFF1_C000), size 124.000 KiB
17:35:43 0 I cpu = 2; stack_page = 34359738109 @ 0v7FFF_FFEF_D000; stack_frame = Frame(32472 @ 0p7ED_8000); stack_flags = PageTableFlags(PRESENT | WRITABLE | ACCESSED | DIRTY)
17:35:43 0 I cpu = 2; stack_guard_page = 34359738108 @ 0v7FFF_FFEF_C000; stack_guard_frame = NoFrame; stack_guard_flags = PageTableFlags(0x0)
17:35:43 0 I cpu = 3; stack_guard = [0v7FFF_FFF3_C000, 0v7FFF_FFF3_D000), size 4.000 KiB; stack = [0v7FFF_FFF3_D000, 0v7FFF_FFF5_C000), size 124.000 KiB
17:35:43 0 I cpu = 3; stack_page = 34359738173 @ 0v7FFF_FFF3_D000; stack_frame = Frame(32408 @ 0p7E9_8000); stack_flags = PageTableFlags(PRESENT | WRITABLE | ACCESSED | DIRTY)
17:35:43 0 I cpu = 3; stack_guard_page = 34359738172 @ 0v7FFF_FFF3_C000; stack_guard_frame = NoFrame; stack_guard_flags = PageTableFlags(0x0)
17:35:43 0 I stack_a = [0v7FFF_FFE7_D000, 0v7FFF_FFE9_C000), size 124.000 KiB; stack_b = [0v7FFF_FFEB_D000, 0v7FFF_FFED_C000), size 124.000 KiB
17:35:43 0 I stack_a = [0v7FFF_FFEB_D000, 0v7FFF_FFED_C000), size 124.000 KiB; stack_b = [0v7FFF_FFEF_D000, 0v7FFF_FFF1_C000), size 124.000 KiB
17:35:43 0 I stack_a = [0v7FFF_FFEF_D000, 0v7FFF_FFF1_C000), size 124.000 KiB; stack_b = [0v7FFF_FFF3_D000, 0v7FFF_FFF5_C000), size 124.000 KiB
3_smp_4_cpus::kernel_stacks------------------------ [passed]
17:35:43 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/smp/cpu.rs | 54 ++++++++++++++++++++++++++++++++++++++++++++++--------
 1 file changed, 46 insertions(+), 8 deletions(-)
```
