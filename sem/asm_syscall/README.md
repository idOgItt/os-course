# `asm_syscall`

В этой задаче мы будем компилировать программы без использования
стандартной библиотеки языка Rust. Многие функции стандартной библиотеки
на самом деле являются обёртками над системными вызовами ядра Linux.
Например, чтобы открыть файл, в конечном счёте используется
системный вызов `open(2)` (цифра 2 означает что документацию на этот
вызов следует искать в секции 2 страниц документации `man`).

Как было рассказано на семинаре, системный вызов можно
выполнить из ассемблера с помощью инструкции `syscall`.
Перед выполнением `syscall` нужно подготовить регистры:
положить на них номер системного вызова и аргументы.

Вам нужно будет написать обёртки для системных вызовов, определённых в файле `src/lib.rs`.
Писать придётся на Rust с использованием ассемблерных вставок с помощью макроса
[`asm!()`](https://doc.rust-lang.org/core/arch/macro.asm.html).
Документацию на который можно посмотреть в
[Rust By Example](https://doc.rust-lang.org/nightly/rust-by-example/unsafe/asm.html) и
[The Rust Reference](https://doc.rust-lang.org/nightly/reference/inline-assembly.html).

Весьма удобная таблица системных вызовов для x86-64 находится по адресу:
<http://blog.rchapman.org/posts/Linux_System_Call_Table_for_x86_64>

Для проверки того что ваши обёртки работают используйте тесты:
`tests/read.rs`, `tests/pipe.rs`, `tests/fork.rs` и `tests/exec.rs`.
Как не сложно догадаться, в них используются эти системные вызовы
(а также некоторые другие). Посмотрите на эти тесты.
Постарайтесь не только добиться того чтобы они работали,
но и понять, что они делают. Учтите, что в тестах не проверяются
коды возврата из системных вызовов. Возможно, вам придётся добавить
таких проверок, чтобы разобраться почему не работает тест.
Также для отладки рекомендуем воспользоваться утилитой `strace`: она показывает какие системные вызовы выполнял процесс (а с ключём `-f` покажут ещё и системные вызовы дочерних процессов).
```console
/.../nikka/sem/asm_syscall$ cargo test
...
     Running tests/exec.rs (/.../nikka/sem/target/debug/deps/exec-00ec6811f87e93da)
...
/.../nikka/sem/asm_syscall$ strace --follow-forks --trace=pipe,fork,read,close,dup,execve,exit /.../nikka/sem/target/debug/deps/exec-00ec6811f87e93da
...
strace: Process 434424 attached
[pid 434424] pipe([3, 4])               = 0
[pid 434424] fork(strace: Process 434425 attached
)                     = 434425
[pid 434424] close(4 <unfinished ...>
[pid 434425] close(3 <unfinished ...>
[pid 434424] <... close resumed>)       = 0
[pid 434425] <... close resumed>)       = 0
[pid 434424] read(3,  <unfinished ...>
[pid 434425] close(1)                   = 0
[pid 434425] dup(4)                     = 1
[pid 434425] execve("/bin/echo", ["/bin/echo", "hi"], 0x7f2f0c000c40 /* 0 vars */) = 0
[pid 434425] close(3)                   = 0
[pid 434425] read(3, "\177ELF\2\1\1\3\0\0\0\0\0\0\0\0\3\0>\0\1\0\0\0P\237\2\0\0\0\0\0"..., 832) = 832
[pid 434425] close(3)                   = 0
[pid 434424] <... read resumed>"hi\n", 10) = 3
[pid 434425] close(1)                   = 0
[pid 434425] close(2)                   = 0
[pid 434425] +++ exited with 0 +++
[pid 434424] --- SIGCHLD {si_signo=SIGCHLD, si_code=CLD_EXITED, si_pid=434425, si_uid=1000, si_status=0, si_utime=0, si_stime=0} ---
[pid 434424] exit(0)                    = ?
[pid 434424] +++ exited with 0 +++
test test_exec ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

+++ exited with 0 +++
```
