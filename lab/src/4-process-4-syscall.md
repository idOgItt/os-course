## Поддержка системных вызовов

Для системных вызовов Nikka использует инструкции
[`syscall` и `sysret`](https://wiki.osdev.org/SYSENTER#AMD:_SYSCALL.2FSYSRET),
[добавленные AMD](https://en.wikipedia.org/wiki/X86_instruction_listings#Added_with_AMD_K6)
специально для [64-битного режима](https://en.wikipedia.org/wiki/Long_mode).
Они [работают в этом режиме и на процессорах Intel](https://en.wikipedia.org/wiki/X86-64#Recent_implementations):

> Intel 64 allows SYSCALL/SYSRET only in 64-bit mode (not in compatibility mode), and allows SYSENTER/SYSEXIT in both modes.
> AMD64 lacks SYSENTER/SYSEXIT in both sub-modes of long mode.

То есть, в [64-битном режиме](https://en.wikipedia.org/wiki/Long_mode)
инструкции [`syscall` и `sysret`](https://wiki.osdev.org/SYSENTER#AMD:_SYSCALL.2FSYSRET)
переносимы, в отличие от похожих инструкций
[`sysenter` и `sysexit`](https://wiki.osdev.org/SYSENTER#INTEL:_SYSENTER.2FSYSEXIT).

Прежде чем приступать, изучите документацию производителя процессоров на нужные инструкции:
- [`syscall`](https://www.felixcloutier.com/x86/syscall),
- [`sysretq`](https://www.felixcloutier.com/x86/sysret),
- [`sti`](https://www.felixcloutier.com/x86/sti).


### Задача 4 --- поддержка системных вызовов


#### Инициализация системных вызовов

Реализуйте [функцию](../../doc/kernel/process/syscall/fn.init.html)

```rust
fn kernel::process::syscall::init()
```

в файле [`kernel/src/process/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/syscall.rs).

Она подготавливает процессор к выполнению инструкций
[`syscall` и `sysret`](https://wiki.osdev.org/SYSENTER#AMD:_SYSCALL.2FSYSRET):

- Включает бит [`x86_64::registers::model_specific::EferFlags::SYSTEM_CALL_EXTENSIONS`](../../doc/x86_64/registers/model_specific/struct.EferFlags.html#associatedconstant.SYSTEM_CALL_EXTENSIONS) в регистре [`x86_64::registers::model_specific::Efer`](../../doc/x86_64/registers/model_specific/struct.Efer.html). А остальные биты оставляет в исходном состоянии.
- Записывает в регистр [`x86_64::registers::model_specific::Star`](../../doc/x86_64/registers/model_specific/struct.Star.html) методом [`Star::write()`](../../doc/x86_64/registers/model_specific/struct.Star.html#method.write) селекторы кода и данных для режимов пользователя и ядра --- [`kernel::memory::gdt::Gdt::user_code()`](../../doc/kernel/memory/gdt/struct.SmpGdt.html#method.user_code), [`kernel::memory::gdt::Gdt::user_data()`](../../doc/kernel/memory/gdt/struct.SmpGdt.html#method.user_data), [`kernel::memory::gdt::Gdt::kernel_code()`](../../doc/kernel/memory/gdt/struct.SmpGdt.html#method.kernel_code), [`kernel::memory::gdt::Gdt::kernel_data()`](../../doc/kernel/memory/gdt/struct.SmpGdt.html#method.kernel_data).
- Записывает в регистр [`x86_64::registers::model_specific::LStar`](../../doc/x86_64/registers/model_specific/struct.LStar.html) виртуальный адрес функции [`kernel::process::syscall::syscall_trampoline()`](../../doc/kernel/process/syscall/fn.syscall_trampoline.html). Получить виртуальный адрес функции можно с помощью конструкции `Virt::from_ptr(syscall_trampoline as *const ())`.
- Записывает в регистр [`x86_64::registers::model_specific::SFMask`](../../doc/x86_64/registers/model_specific/struct.SFMask.html) маску для регистра флагов `RFLAGS`, которая определяет какие флаги в `RFLAGS` будут сброшены при входе в системный вызов. Нужно сбросить флаг прерываний. Так как если прерывание возникнет сразу после переключения в ядро и до того как ядро переключится в собственный стек, процессор сохранит контекст прерывания на пользовательский стек. А ему, как мы помним, доверять нельзя. Также предлагается сбросить все остальные флаги, просто для определённости состояния `RFLAGS` в момент системного вызова.


#### Диспетчеризация системных вызовов

Реализуйте на ассемблере [функцию](../../doc/kernel/process/syscall/fn.syscall_trampoline.html)

```rust
extern "C" fn syscall_trampoline() -> !
```

в файле [`kernel/src/process/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/syscall.rs).

Она получает управление при выполнении инструкции `syscall`.
Свои аргументы эта функция принимает от кода пользователя строго через регистры.
Вы можете сделать ей собственный ABI, или же ориентироваться на существующий.
Например, на
[Linux x86-64 syscall ABI](https://refspecs.linuxbase.org/elf/x86_64-abi-0.99.pdf#subsection.A.2.1).
Функция
[`syscall_trampoline()`](../../doc/kernel/process/syscall/fn.syscall_trampoline.html)
должна передать управление в написанную на Rust
[функцию](../../doc/kernel/process/syscall/fn.syscall.html)

```rust
#[no_mangle]
extern "C" fn kernel::process::syscall::syscall(
    // https://wiki.osdev.org/System_V_ABI#x86-64:
    // Parameters to functions are passed in the registers rdi, rsi, rdx, rcx, r8, r9, and further values are passed on the stack in reverse order.
    number: usize, // rdi
    arg0: usize,   // rsi
    arg1: usize,   // rdx
    rip: Virt,     // rcx
    rsp: Virt,     // r8
    arg2: usize,   // r9
    // Stack, push in reverse order.
    arg3: usize,
    arg4: usize,
) -> !
```

Указание
[`extern "C"`](https://doc.rust-lang.ru/book/ch19-01-unsafe-rust.html#%D0%98%D1%81%D0%BF%D0%BE%D0%BB%D1%8C%D0%B7%D0%BE%D0%B2%D0%B0%D0%BD%D0%B8%D0%B5-extern-%D1%84%D1%83%D0%BD%D0%BA%D1%86%D0%B8%D0%B9-%D0%B4%D0%BB%D1%8F-%D0%B2%D1%8B%D0%B7%D0%BE%D0%B2%D0%B0-%D0%B2%D0%BD%D0%B5%D1%88%D0%BD%D0%B5%D0%B3%D0%BE-%D0%BA%D0%BE%D0%B4%D0%B0)
в её сигнатуре означает, что она подчиняется соглашениям
[C ABI](https://wiki.osdev.org/System_V_ABI#x86-64) текущей архитектуры.
Подробно про них можно посмотреть в
[System V Application Binary InterfaceAMD64 Architecture Processor Supplement](https://github.com/hjl-tools/x86-psABI/wiki/x86-64-psABI-draft.pdf).

Аннотация
[`#[no_mangle]`](https://doc.rust-lang.org/reference/abi.html#the-no_mangle-attribute)
означает, что имя функции, доступное в ассемблере будет как написано в коде --- `syscall`.
Иначе оно будет искажено для уникализации, --- чтобы не совпадать с таким же именем в другом модуле.
(В C++ искажение имени учитывает ещё и типы аргументов для реализации перегрузки функций.)
Без такой аннотации в ассемблере имя `syscall()` было бы похоже на
`_ZN6kernel7process7syscall7syscall17hc1ae395af68eb49cE`.
Программа `rustfilt` позволяет восстановить искажённое имя:
```console
$ rustfilt _ZN6kernel7process7syscall7syscall17hc1ae395af68eb49cE
kernel::process::syscall::syscall
```
Чтобы не писать в ассемблере что-нибудь вроде
`call _ZN6kernel7process7syscall7syscall17hc1ae395af68eb49cE`,
используем аннотацию
[`#[no_mangle]`](https://doc.rust-lang.org/reference/abi.html#the-no_mangle-attribute).
Но тогда мы сами должны гарантировать уникальность имени, как в ассемблере или C.

На момент входа в [`syscall_trampoline()`](../../doc/kernel/process/syscall/fn.syscall_trampoline.html)
регистр `RSP` указывает на стек пользователя.
Мы не можем ему доверять, --- код режима пользователя мог его переполнить, неправильно выровнять, поместить на недоступный для пользователя адрес, например внутрь данных или кода ядра.
Поэтому прежде чем пользоваться стеком,
[`syscall_trampoline()`](../../doc/kernel/process/syscall/fn.syscall_trampoline.html)
переключается в стек ядра.
Адрес пользовательского стека --- старое значение `RSP` --- она должна передать в функцию
[`syscall()`](../../doc/kernel/process/syscall/fn.syscall.html)
в соответствующем [x86-64 C ABI](https://wiki.osdev.org/System_V_ABI#x86-64) регистре.

Помните, что благодаря маске в регистре
[`x86_64::registers::model_specific::SFMask`](../../doc/x86_64/registers/model_specific/struct.SFMask.html),
процессор выключил прерывания в момент выполнения инструкции
[`SYSCALL`](https://www.felixcloutier.com/x86/syscall)?
Nikka сама по себе не сломается от получения прерывания в момент выполнения системного вызова,
и мы уже на стеке ядра --- самое время включить прерывания с помощью инструкции
[`STI`](https://www.felixcloutier.com/x86/sti).
Если же не включить прерывания, можно например пропустить очередной тик RTC, пока будет выполняться системный вызов.
И тогда в логе может быть неверное время.
Например, тут последовательные строки логирования выглядят как будто между ними прошло 15-16 секунд:

```console
10:48:24 0 D entering the user mode; pid = 0:4; registers = { rax: 0x0, rdi: 0x7F7FFFFAD000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v10007550, ss:rsp: 0x001B:0v7F7FFFFFF000, rflags: IF } }
10:48:40.001 0 D leaving the user mode; pid = 0:4
```

А на самом деле было пропущено много RTC прерываний.
Вообще, после инициализации RTC было получено только первое прерывание в 10:48:24.
И подсистема времени всё ещё ориентируется на одну единственную базовую точку в
[`SystemInfo::rtc`](../../doc/ku/info/struct.SystemInfo.html#structfield.rtc),
которая этому первому прерыванию соответствует.
И только в 10:48:40 было получено второе прерывание.
А все промежуточные пропали из-за того, что во время исполнения системных вызовов прерывания были отключены.

То что прерывания запрещены до переключения стека, проверяется в тесте `4-process-4-syscall` из файла
[`kernel/tests/4-process-4-syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-4-syscall.rs)
следующим образом.
Код пользователя из файла
[`user/exit/src/main.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/exit/src/main.rs)
запускается при запрещённых прерываниях.
Он ждёт 100 миллисекунд, этого должно быть достаточно чтобы накопилось несколько тиков
[PIT](https://en.wikipedia.org/wiki/Programmable_interval_timer):
```rust
// Wait for some PIT ticks so an interrupt will be pending on the return to the kernel mode.
// This test is run with interrupts disabled.
// If the kernel enables interrupts before switching the stack it will receive a Page Fault.
time::delay(Duration::milliseconds(100));
```
А значит, появился сигнал о прерывании.
После этого код пользователя записывает `0` в `RSP` и делает системный вызов.
Начинает исполняться код ядра.
И как только он разрешит прерывания, то сразу же получит прерывание от
[PIT](https://en.wikipedia.org/wiki/Programmable_interval_timer).

Если стек ещё не был переключён, это приведёт к Page Fault из-за того что код пользователя испортил `RSP`:

```console
$ (cd kernel; cargo test --test 4-process-4-syscall)
...
4_process_4_syscall::syscall_exit-----------------------------
...
19:32:14 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFEB000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v10007BA0, ss:rsp: 0x001B:0v7F7FFFFFF000, rflags:  } }
19:32:14 0 D trap = "Page Fault"; context = { mode: kernel, cs:rip: 0x0008:0v8648E4, ss:rsp: 0x0010:0v0, rflags: IF }; info = { code: 0b11 = protection violation | write | kernel, address: 0vFFFFFFFFFFFFFFF8 }
19:32:14 0 E kernel mode trap; trap = "Page Fault"; number = 14; info = { code: 0b11 = protection violation | write | kernel, address: 0vFFFFFFFFFFFFFFF8 }; context = { mode: kernel, cs:rip: 0x0008:0v8648E4, ss:rsp: 0x0010:0v0, rflags: IF }
panicked at 'kernel mode trap #14 - Page Fault, context: { mode: kernel, cs:rip: 0x0008:0v8648E4, ss:rsp: 0x0010:0v0, rflags: IF }', kernel/src/trap.rs:411:13
--------------------------------------------------- [failed]
```

После переключения стека, аргументы функции
[`syscall()`](../../doc/kernel/process/syscall/fn.syscall.html),
которые по [x86-64 C ABI](https://wiki.osdev.org/System_V_ABI#x86-64)
должны передаваться через стек,
[`syscall_trampoline()`](../../doc/kernel/process/syscall/fn.syscall_trampoline.html)
должна записать в стек.
После этого она должна вызвать функцию
[`syscall()`](../../doc/kernel/process/syscall/fn.syscall.html).
Так как
[`syscall()`](../../doc/kernel/process/syscall/fn.syscall.html)
не возвращается, сохранять адрес возврата в стеке инструкцией
[`call`](https://www.felixcloutier.com/x86/call)
не обязательно, можно сделать
[`jmp`](https://www.felixcloutier.com/x86/jmp).
Но если вы предпочли вариант с
[`jmp`](https://www.felixcloutier.com/x86/jmp),
то скорректируйте значение регистра `RSP`.
Так как вызываемая функция при поиске аргументов, передаваемых через стек,
пропускает в нём место с адресом возврата.
То есть, считает что адрес возврата был сохранён инструкцией
[`call`](https://www.felixcloutier.com/x86/call).

Реализуйте функцию [`syscall()`](../../doc/kernel/process/syscall/fn.syscall.html).
Она выполняет диспетчеризацию системных вызовов по аргументу `number`,
передавая в реализующие конкретные системные вызовы функции нужную часть аргументов `arg0`--`arg4`.
Пока что нам хватит системных вызовов

- [`kernel::process::syscall::exit()`](../../doc/kernel/process/syscall/fn.exit.html) с номером [`ku::process::syscall::Syscall::Exit`](../../doc/ku/process/syscall/enum.Syscall.html#variant.Exit) и
- [`kernel::process::syscall::log_value()`](../../doc/kernel/process/syscall/fn.log_value.html) с номером [`ku::process::syscall::Syscall::LogValue`](../../doc/ku/process/syscall/enum.Syscall.html#variant.LogValue).

Вам может пригодиться структура
[`kernel::smp::cpu::Cpu`](../../doc/kernel/smp/cpu/struct.Cpu.html)
и её методы.

После выполнения функции, реализующей нужный системный вызов,
[`syscall()`](../../doc/kernel/process/syscall/fn.syscall.html)
передаёт управление в функцию
[`kernel::process::syscall::sysret()`](../../doc/kernel/process/syscall/fn.sysret.html).


#### Возврат из системного вызова

Реализуйте на ассемблере [функцию](../../doc/kernel/process/syscall/fn.sysret.html)

```rust
fn kernel::process::syscall::sysret(
    context: MiniContext,
    result: ResultCode,
    res: SyscallResult,
) -> !
```

в файле [`kernel/src/process/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/syscall.rs).

Она

- Записывает результат системного вызова `res` в регистры общего назначения, например в `RAX`, `RDI`, `RSI`, и т.д. Обратите внимание, что признак и код ошибки, если `result` содержит ошибку, нужно записать в один из регистров --- пользовательскому процессу он тоже важен. Придумайте свой ABI или руководствуйтесь [Linux x86-64 syscall ABI](https://refspecs.linuxbase.org/elf/x86_64-abi-0.99.pdf#subsection.A.2.1). При этом безусловно сохранять какие-либо регистры пользователя представляется лишней работой. Лучше доверить задачу определения сохраняемых регистров компилятору. А в ABI считать, что все регистры модифицированы.
- Записывает в регистр `R11` состояние регистра флагов, которое должно быть в пространстве пользователя. Как минимум, должен быть установлен [`RFlags::INTERRUPT_FLAG`](../../doc/ku/process/registers/struct.RFlags.html#associatedconstant.INTERRUPT_FLAG), чтобы процесс не мог монополизировать процессор. Но пока что включение прерываний приведёт к нестабильности теста `4_process_4_syscall::syscall_log_value`. Поэтому предлагается отложить включение [`RFlags::INTERRUPT_FLAG`](../../doc/ku/process/registers/struct.RFlags.html#associatedconstant.INTERRUPT_FLAG) до одной из следующих задач. А пока --- выключить все флаги при возврате в режим пользователя, воспользовавшись [`RFlags::default()`](../../doc/ku/process/registers/struct.RFlags.html#method.default).
- Записывает адрес возврата в код пользователя в регистр `RCX`. Функции [`syscall_trampoline()`](../../doc/kernel/process/syscall/fn.syscall_trampoline.html)/[`syscall()`](../../doc/kernel/process/syscall/fn.syscall.html) получили этот адрес в том же регистре. И должны были передать его вместе с адресом стека пользователя в аргументе `context`.
- Переключает регистр `RSP` на стек пользователя.
- Зануляет все неиспользованные выше регистры общего назначения, чтобы предотвратить утечку информации из режима ядра в режим пользователя.
- Выполняет инструкцию [`sysretq`](https://www.felixcloutier.com/x86/sysret), которая передаёт управление в код пользователя в соответствии с настройками в регистрах `Star`, `R11` и `RCX`. Обратите внимание на суффикс `q` у [`sysretq`](https://www.felixcloutier.com/x86/sysret). Если его не указать, ассемблер сгенерирует машинный код для другого режима работы процессора и до некоторого момента это будет не заметно, так как делать он будет почти то же самое. А потом в неожиданный момент всё сломается и найти такую ошибку будет тяжело.


#### Системный вызов `exit`

Реализуйте [функцию](../../doc/kernel/process/syscall/fn.exit.html)

```rust
fn kernel::process::syscall::exit(
    process: SpinlockGuard<Process>,
    code: usize,
) -> !
```

в файле [`kernel/src/process/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/syscall.rs).

Она выполняет системный вызов `exit(code)` с одним аргументом `code`.
Код можно, например, залогировать как
[`ku::process::syscall::ExitCode`](../../doc/ku/process/syscall/enum.ExitCode.html).
Основная же работа этого системного вызова заключается в освобождении слота таблицы процессов методом
[`kernel::process::table::Table::free()`](../../doc/kernel/process/table/struct.Table.html#method.free)
и возврате в контекст ядра, из которого пользовательский процесс был запущен.
Это делается статическим методом
[`kernel::process::process::Process::sched_yield()`](../../doc/kernel/process/process/struct.Process.html#method.sched_yield),
которые не возвращают управление, как и сама функция
[`kernel::process::syscall::exit()`](../../doc/kernel/process/syscall/fn.exit.html).


#### Системный вызов `log_value`

Реализуйте [функцию](../../doc/kernel/process/syscall/fn.log_value.html)

```rust
fn kernel::process::syscall::log_value(
    process: SpinlockGuard<Process>,
    str_start: usize,
    str_end: usize,
    value: usize,
) -> Result<SyscallResult>
```

в файле [`kernel/src/process/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/src/process/syscall.rs).

Это системный вызов, который принимает три аргумента.
В аргументах `str_start` и `str_end` передаётся начало и конец строки типа [`&str`](https://doc.rust-lang.org/nightly/core/primitive.str.html), а в `value` --- произвольное число.

Для проверки корректности пары `str_start` и `str_end` вам пригодятся функции
[`ku::memory::block::Block::<Virt>::from_index()`](../../doc/ku/memory/block/struct.Block.html#method.from_index),
[`kernel::memory::address_space::AddressSpace::check_permission()`](../../doc/kernel/memory/address_space/struct.AddressSpace.html#method.check_permission) и
[`core::str::from_utf8()`](https://doc.rust-lang.org/nightly/core/str/fn.from_utf8.html).
Если хотя бы одна из них вернула ошибку, прокиньте её оператором `?` в функцию
[`syscall()`](../../doc/kernel/process/syscall/fn.syscall.html),
чтобы она через функцию
[`sysret()`](../../doc/kernel/process/syscall/fn.sysret.html)
вернула эту ошибку в код пользователя.
Ошибку [`core::str::Utf8Error`](https://doc.rust-lang.org/nightly/core/str/struct.Utf8Error.html)
можно преобразовать, например, в
[`ku::error::Error::InvalidArgument`](../../doc/ku/error/enum.Error.html#variant.InvalidArgument).

Далее системный вызов
[`log_value()`](../../doc/kernel/process/syscall/fn.log_value.html)
делает свою основную работу --- логирует получившуюся строку и `value`.
Вам будет удобнее пользоваться
[`log_value()`](../../doc/kernel/process/syscall/fn.log_value.html)
, если вы залогируете `value` и в десятичном, и в шестнадцатеричном виде.
После этого системный вызов возвращает управление.

С помощью этого системного вызова тест `4-process-4-syscall` из файла
[`kernel/tests/4-process-4-syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-4-syscall.rs)
проверяет возможность чтения системного времени из непривилегированного режима пользователя.
Код в файле
[`user/log_value/src/main.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/log_value/src/main.rs)
выполняет вызов `time::now()` и логирует полученное количество секунд с unix-эпохи:

```rust
let now = time::now();
let timestamp = now.timestamp().try_into().unwrap();

if syscall::log_value("user space can read the system time", timestamp).is_err() {
    generate_page_fault();
}
```

Должно залогироваться что-то подобное:

```console
$ (cd kernel; cargo test --test 4-process-4-syscall)
...
4_process_4_syscall::syscall_log_value------------------------
...
04:30:41 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFEB000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v10008BD0, ss:rsp: 0x001B:0v7F7FFFFFF000, rflags:  } }
04:30:41 0 I user space can read the system time; value = 1666585841; hex_value = 0x635614F1; pid = 0:0
...
```

Если в первой лабораторке вы попытались воспользоваться
[приёмом "read-dont-modify-write"](https://web.archive.org/web/20230318132751/https://web.archive.org/web/20230318132751/https://www.hpl.hp.com/techreports/2012/HPL-2012-68.pdf),
то в режиме пользователя и вызов `time::delay()` из
[`user/exit/src/main.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/exit/src/main.rs)
и вызов `time::now()` из
[`user/log_value/src/main.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/log_value/src/main.rs)
получат Page Fault:

```console
$ (cd kernel; cargo test --test 4-process-4-syscall)
...
4_process_4_syscall::syscall_exit-----------------------------
...
04:55:08 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFEB000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v10007BB0, ss:rsp: 0x001B:0v7F7FFFFFF000, rflags:  } }
04:55:08 0 D trap = "Page Fault"; context = { mode: user, cs:rip: 0x0023:0v1000E09C, ss:rsp: 0x001B:0v7F7FFFFFEA78, rflags: PF }; info = { code: 0b111 = protection violation | write | user, address: 0v7F7FFFFEC038 }
04:55:08 0 D leaving the user mode; pid = 0:0
panicked at 'if the Page Fault was in the kernel mode, probably the `syscall` instruction is not initialized or the kernel has not switched to its own stack; if it was in the user mode, maybe the time functions from the first lab use `read-dont-modify-write` construction', kernel/tests/4-process-4-syscall.rs:71:5
--------------------------------------------------- [failed]
```


#### Пользовательская сторона системного вызова

Реализуйте [функцию](../../doc/lib/syscall/fn.syscall.html)

```rust
fn lib::syscall::syscall(
    number: usize,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
) -> Result<(usize, usize)>
```

в файле [`user/lib/src/syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/lib/src/syscall.rs).

Она работает в режиме пользователя.
Её задача:

- Сохранить регистры `RBX` и `RBP` на стеке.
- Передать свои аргументы через регистры, в которых их ожидают функции
[`kernel::process::syscall::syscall_trampoline()`](../../doc/kernel/process/syscall/fn.syscall_trampoline.html) и
[`kernel::process::syscall::syscall()`](../../doc/kernel/process/syscall/fn.syscall.html).
- Запустить инструкцию [`syscall`](https://www.felixcloutier.com/x86/syscall), которая выполнит требуемый системный вызов.
- Восстановить `RBX` и `RBP`.
- Вернуть наружу результаты системного вызова из регистров, в которые их сохранила функция [`kernel::process::syscall::sysret()`](../../doc/kernel/process/syscall/fn.sysret.html).
- Неиспользованные регистры общего назначения она должна пометить как испорченные уже знакомой конструкцией `lateout(...) _,`.


### Проверьте себя

Запустите тесты `4-process-4-syscall` и `4-process-4-log-value` из файлов
[`kernel/tests/4-process-4-syscall.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-4-syscall.rs) и
[`kernel/tests/4-process-4-log-value.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/kernel/tests/4-process-4-log-value.rs):

```console
$ (cd kernel; cargo test --test 4-process-4-syscall --test 4-process-4-log-value)
...
4_process_4_log_value::log_value_implementation-------------
20:37:26 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:37:26 0 I duplicate; address_space = "process" @ 0p7E9_9000
20:37:26 0 I switch to; address_space = "process" @ 0p7E9_9000
20:37:26 0 D extend mapping; block = [0v1000_0000, 0v1000_8234), size 32.551 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:37:26 0 D elf loadable program header; file_block = [0v20_3000, 0v20_B234), size 32.551 KiB; memory_block = [0v1000_0000, 0v1000_8234), size 32.551 KiB
20:37:26 0 D extend mapping; block = [0v1000_9000, 0v1004_F303), size 280.753 KiB; page_block = [0v1000_9000, 0v1005_0000), size 284.000 KiB
20:37:26 0 D elf loadable program header; file_block = [0v20_B240, 0v25_2303), size 284.190 KiB; memory_block = [0v1000_8240, 0v1004_F303), size 284.190 KiB
20:37:26 0 D elf loadable program header; file_block = [0v25_2308, 0v25_23D0), size 200 B; memory_block = [0v1004_F308, 0v1004_F3D0), size 200 B
20:37:26 0 D extend mapping; block = [0v1005_0000, 0v1005_5F90), size 23.891 KiB; page_block = [0v1005_0000, 0v1005_6000), size 24.000 KiB
20:37:26 0 D elf loadable program header; file_block = [0v25_23D0, 0v25_8F68), size 26.898 KiB; memory_block = [0v1004_F3D0, 0v1005_5F90), size 26.938 KiB
20:37:26 0 I switch to; address_space = "base" @ 0p1000
20:37:26 0 I loaded ELF file; context = { rip: 0v1000_8F60, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.613 MiB; process = { pid: <current>, address_space: "process" @ 0p7E9_9000, { rip: 0v1000_8F60, rsp: 0v7F7F_FFFF_F000 } }
20:37:26 0 I user process page table entry; entry_point = 0v1000_8F60; frame = Frame(32321 @ 0p7E4_1000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:37:26 0 D process_frames = 148
20:37:26 0 I switch to; address_space = "0:0" @ 0p7E9_9000
20:37:26 0 I https://en.wikipedia.org/wiki/Pi#/media/File:Pi_pie2.jpg; value = 3141592653589793238; hex_value = 0x2B992DDFA23249D6; pid = 0:0
20:37:26 0 I ; value = 0; hex_value = 0x0; pid = 0:0
20:37:26 0 D dropping; spinlock = kernel/tests/4-process-4-log-value.rs:36:19; stats = Stats { failures: 0, locks: 11, unlocks: 11, waits: 0 }
20:37:26 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 1, unlocks: 1, waits: 0 }
20:37:26 0 I switch to; address_space = "base" @ 0p1000
20:37:26 0 I drop the current address space; address_space = "0:0" @ 0p7E9_9000; switch_to = "base" @ 0p1000
4_process_4_log_value::log_value_implementation---- [passed]
20:37:26 0 I exit qemu; exit_code = ExitCode(SUCCESS)
...
4_process_4_syscall::syscall_exit---------------------------
20:37:34 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:37:34 0 I duplicate; address_space = "process" @ 0p7E9_9000
20:37:34 0 I switch to; address_space = "process" @ 0p7E9_9000
20:37:34 0 D extend mapping; block = [0v1000_0000, 0v1000_8974), size 34.363 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:37:34 0 D elf loadable program header; file_block = [0v20_3000, 0v20_B974), size 34.363 KiB; memory_block = [0v1000_0000, 0v1000_8974), size 34.363 KiB
20:37:34 0 D extend mapping; block = [0v1000_9000, 0v1005_3B4B), size 298.823 KiB; page_block = [0v1000_9000, 0v1005_4000), size 300.000 KiB
20:37:34 0 D elf loadable program header; file_block = [0v20_B980, 0v25_6B4B), size 300.448 KiB; memory_block = [0v1000_8980, 0v1005_3B4B), size 300.448 KiB
20:37:34 0 D elf loadable program header; file_block = [0v25_6B50, 0v25_6C18), size 200 B; memory_block = [0v1005_3B50, 0v1005_3C18), size 200 B
20:37:34 0 D extend mapping; block = [0v1005_4000, 0v1005_ACA0), size 27.156 KiB; page_block = [0v1005_4000, 0v1005_B000), size 28.000 KiB
20:37:34 0 D elf loadable program header; file_block = [0v25_6C18, 0v25_DC78), size 28.094 KiB; memory_block = [0v1005_3C18, 0v1005_ACA0), size 28.133 KiB
20:37:34 0 I switch to; address_space = "base" @ 0p1000
20:37:34 0 I loaded ELF file; context = { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.661 MiB; process = { pid: <current>, address_space: "process" @ 0p7E9_9000, { rip: 0v1000_9A40, rsp: 0v7F7F_FFFF_F000 } }
20:37:34 0 I user process page table entry; entry_point = 0v1000_9A40; frame = Frame(32317 @ 0p7E3_D000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:37:34 0 I switch to; address_space = "0:0" @ 0p7E9_9000
20:37:34 0 D switched to address_space
20:37:34 0 D set pid
20:37:34 0 D set current_process
20:37:34 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1000_9A40, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags:  } }
20:37:35.003 0 I syscall = "exit"; pid = 0:0; code = 3141592653589793238; reason = Err(TryFromPrimitiveError { number: 3141592653589793238 })
20:37:35.011 0 D leaving the user mode; pid = 0:0
4_process_4_syscall::syscall_exit------------------ [passed]

4_process_4_syscall::syscall_log_value----------------------
20:37:35.561 0 I page allocator init; free_page_count = 33822867456; block = [0v180_0000_0000, 0v7F80_0000_0000), size 126.000 TiB
20:37:35.567 0 I duplicate; address_space = "process" @ 0p7DC_4000
20:37:35.571 0 I switch to; address_space = "process" @ 0p7DC_4000
20:37:35.577 0 D extend mapping; block = [0v1000_0000, 0v1000_88B4), size 34.176 KiB; page_block = [0v1000_0000, 0v1000_9000), size 36.000 KiB
20:37:35.583 0 D elf loadable program header; file_block = [0v8A_E000, 0v8B_68B4), size 34.176 KiB; memory_block = [0v1000_0000, 0v1000_88B4), size 34.176 KiB
20:37:35.609 0 D extend mapping; block = [0v1000_9000, 0v1005_14D3), size 289.206 KiB; page_block = [0v1000_9000, 0v1005_2000), size 292.000 KiB
20:37:35.617 0 D elf loadable program header; file_block = [0v8B_68C0, 0v8F_F4D3), size 291.019 KiB; memory_block = [0v1000_88C0, 0v1005_14D3), size 291.019 KiB
20:37:35.631 0 D elf loadable program header; file_block = [0v8F_F4D8, 0v8F_F5A0), size 200 B; memory_block = [0v1005_14D8, 0v1005_15A0), size 200 B
20:37:35.639 0 D extend mapping; block = [0v1005_2000, 0v1005_84B0), size 25.172 KiB; page_block = [0v1005_2000, 0v1005_9000), size 28.000 KiB
20:37:35.647 0 D elf loadable program header; file_block = [0v8F_F5A0, 0v90_6488), size 27.727 KiB; memory_block = [0v1005_15A0, 0v1005_84B0), size 27.766 KiB
20:37:35.677 0 I switch to; address_space = "base" @ 0p1000
20:37:35.679 0 I loaded ELF file; context = { rip: 0v1000_AB50, rsp: 0v7F7F_FFFF_F000 }; file_size = 6.666 MiB; process = { pid: <current>, address_space: "process" @ 0p7DC_4000, { rip: 0v1000_AB50, rsp: 0v7F7F_FFFF_F000 } }
20:37:35.691 0 D dropping; spinlock = kernel/src/process/table.rs:268:36; stats = Stats { failures: 0, locks: 2, unlocks: 2, waits: 0 }
20:37:35.697 0 D dropping; spinlock = kernel/src/process/process.rs:65:28; stats = Stats { failures: 0, locks: 1, unlocks: 1, waits: 0 }
20:37:35.703 0 I drop; address_space = "0:0" @ 0p7E9_9000
20:37:36.687 0 I user process page table entry; entry_point = 0v1000_AB50; frame = Frame(32160 @ 0p7DA_0000); flags = PageTableFlags(PRESENT | WRITABLE | USER_ACCESSIBLE | ACCESSED | DIRTY)
20:37:36.695 0 I switch to; address_space = "0:0" @ 0p7DC_4000
20:37:36.699 0 D switched to address_space
20:37:36.703 0 D set pid
20:37:36.705 0 D set current_process
20:37:36.709 0 D entering the user mode; pid = 0:0; registers = { rax: 0x0, rdi: 0x7F7FFFFD5000, rsi: 0x0, { mode: user, cs:rip: 0x0023:0v1000_AB50, ss:rsp: 0x001B:0v7F7F_FFFF_F000, rflags:  } }
20:37:36.725 0 I user space can read the system time; value = 1698881856; hex_value = 0x6542E140; pid = 0:0
20:37:36.733 0 I ; value = 0; hex_value = 0x0; pid = 0:0
20:37:36.739 0 W syscall failed; syscall = Ok(LogValue); number = 1; arg0 = 1; arg1 = 1; arg2 = 0; arg3 = 0; arg4 = 0; error = PermissionDenied
20:37:36.747 0 W syscall failed; syscall = Ok(LogValue); number = 1; arg0 = 0; arg1 = 0; arg2 = 0; arg3 = 0; arg4 = 0; error = Null
20:37:36.757 0 W syscall failed; syscall = Ok(LogValue); number = 1; arg0 = 268439768; arg1 = 1; arg2 = 0; arg3 = 0; arg4 = 0; error = InvalidArgument
20:37:36.765 0 W syscall failed; syscall = Ok(LogValue); number = 1; arg0 = 65536; arg1 = 18446744069414584320; arg2 = 0; arg3 = 0; arg4 = 0; error = InvalidArgument
20:37:36.775 0 W syscall failed; syscall = Ok(LogValue); number = 1; arg0 = 18446744073709486080; arg1 = 1048576; arg2 = 0; arg3 = 0; arg4 = 0; error = Overflow
20:37:36.783 0 I syscall = "exit"; pid = 0:0; code = 0; reason = Ok(Ok)
20:37:36.787 0 D leaving the user mode; pid = 0:0
4_process_4_syscall::syscall_log_value------------- [passed]
20:37:36.793 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```

Если вы увидите в тесте `4_process_4_syscall::syscall_log_value`
Page Fault на чтении `0v1` в режиме пользователя (не ядра),
это просто замена `panic!()` в коде пользователя в тесте.
В логе выше этого Page Fault должно быть написано, что ему не нравится,
если конечно `log_value()` хоть немного работает.
Например, --- `expected Err(InvalidArgument), got Ok`:

```console
...
06:00:33 0 I expected Err(InvalidArgument), got Ok; value = 0; hex_value = 0x0; pid = 0:0
06:00:33 0 D trap = "Page Fault"; context = { mode: user, cs:rip: 0x0023:0v10012E32, ss:rsp: 0x001B:0v7F7FFFFFEA38, rflags: AF PF }; info = { address: 0v1, code: 0b100 = non-present page | read | user }
06:00:33 0 D leaving the user mode; pid = 0:0
panicked at 'the user mode code has detected an error in syscall::log_value() implementation', kernel/tests/4-process-4-syscall.rs:60:5
--------------------------------------------------- [failed]
```

Либо можно по коду теста в файле
[`user/log_value/src/main.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/user/log_value/src/main.rs)
посмотреть какая проверка не прошла.


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/process/syscall.rs | 151 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++--------
 user/lib/src/syscall.rs       |  57 +++++++++++++++++++++++++++++--
 2 files changed, 192 insertions(+), 16 deletions(-)
```
