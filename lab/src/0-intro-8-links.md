## Ссылки


{{#include ../../README.md}}


### Операционные системы

- [JOS](https://pdos.csail.mit.edu/6.828/2018/labs/lab1/), разработанная в MIT специально для курса [6.828: Operating System Engineering](https://pdos.csail.mit.edu/6.828/2018/index.html).
- Учебная Unix-подобная ОС [xv6](https://en.wikipedia.org/wiki/Xv6):
  - [textbook](https://pdos.csail.mit.edu/6.828/2018/xv6/book-rev11.pdf),
  - [booklet](https://pdos.csail.mit.edu/6.828/2018/xv6/xv6-rev11.pdf),
  - [source](https://github.com/mit-pdos/xv6-public),
  - [homepage](https://pdos.csail.mit.edu/6.828/2018/xv6.html).
- [Modern Operating Systems by Andrew S. Tanenbaum and Herbert Bos](https://www.amazon.com/Modern-Operating-Systems-Global-Edition/dp/1292459662/).
- [Operating Systems Design and Implementation by Andrew S. Tanenbaum and Albert S. Woodhull](https://www.amazon.com/Operating-Systems-Design-Implementation-3rd/dp/0131429388) и сама учебная Unix-подобная ОС [MINIX 3](https://www.minix3.org/).
- [Семейство ОС L4](https://en.wikipedia.org/wiki/L4_microkernel_family):
  - [Oфициальный сайт](http://os.inf.tu-dresden.de/L4/overview.html).
  - [Верифицированная OS seL4](https://sel4.systems/).
- [Writing an OS in Rust, Philipp Oppermann's blog](https://os.phil-opp.com/).
- [The Adventures of OS: Making a RISC-V Operating System using Rust, Stephen Marz: Blog](https://osblog.stephenmarz.com/).
- [Operating Systems: Three Easy Pieces by Remzi H. Arpaci-Dusseau and Andrea C. Arpaci-Dusseau](https://pages.cs.wisc.edu/~remzi/OSTEP/).
- [The little book about OS development by Erik Helin and Adam Renberg](https://littleosbook.github.io/).
- [Инструкция по написанию ОС](http://www.brokenthorn.com/Resources/OSDevIndex.html), достаточно подробно описывающая устройства.
- [The managarm OS](https://github.com/managarm/managarm) --- ОС с полностью асинхронным API.
- [Redox OS](https://doc.redox-os.org/book/) --- Unix-подобная ОС на Rust.


### Форматы и ABI

- [x86-64 Application Binary Interface](https://github.com/hjl-tools/x86-psABI/wiki/x86-64-psABI-1.0.pdf).
- [Linux system call table for x86_64](http://blog.rchapman.org/posts/Linux_System_Call_Table_for_x86_64).
- [ELF64 binary format](https://www.uclibc.org/docs/elf-64-gen.pdf).


### Ассемблер

- Наиболее полные руководства по архитектуре:
  - [AMD64 Architecture Programmer’s Manual](https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/40332.pdf),
  - [Intel® 64 and IA-32 Architectures Software Developer’s Manual](https://software.intel.com/en-us/download/intel-64-and-ia-32-architectures-sdm-combined-volumes-1-2a-2b-2c-2d-3a-3b-3c-3d-and-4).
- Удобный справочник по командам ассемблера: [x86 and amd64 instruction reference](https://www.felixcloutier.com/x86/).
- Макрос [`asm!()`](https://doc.rust-lang.org/core/arch/macro.asm.html): [Rust By Example: Inline assembly](https://doc.rust-lang.org/nightly/rust-by-example/unsafe/asm.html) и [The Rust Reference: Inline assembly](https://doc.rust-lang.org/nightly/reference/inline-assembly.html).
- [Agner Fog's Software optimization resources](https://agner.org/optimize).
- [Время выполнения x86 инструкций](https://uops.info).
- Методичка ВМК МГУ для певокурсников:
  - [Семинары по курсу "Архитектура ЭВМ и язык ассемблера" Часть 1, Е.А. Кузьменкова, В.С. Махнычев, В.А. Падарян](http://asmcourse.cs.msu.ru/wp-content/uploads/2015/03/asm-ucebnice-1.pdf),
  - [Семинары по курсу "Архитектура ЭВМ и язык ассемблера" Часть 2, Е.А. Кузьменкова, В.А. Падарян, М.А. Соловьев](http://asmcourse.cs.msu.ru/wp-content/uploads/2015/02/asm-ucebnice-2.pdf).


### Архитектура компьютера

- [Digital Design and Computer Architecture by David Harris and Sarah Harris](https://www.amazon.com/Digital-Design-Computer-Architecture-Harris/dp/0123944244).
- [The Manga Guide to Microprocessors by Michio Shibuya, Takashi Tonagi, and Office Sawa](https://www.amazon.com/Manga-Guide-Microprocessors-Michio-Shibuya/dp/1593278179).
- [What every programmer should know about memory by Ulrich Drepper](http://people.redhat.com/drepper/cpumemory.pdf).


### Rust

- Курс ШАДа [Современное системное программирование на Rust](https://lk.yandexdataschool.ru/courses/2021-autumn/7.939-rust/classes/).
- [Официальный сайт Rust](https://www.rust-lang.org/).
- [The Rust Programming Language](https://doc.rust-lang.org/book/) --- основная книга по языку, [перевод](https://doc.rust-lang.ru/book/).
- [The Rust Reference](https://doc.rust-lang.org/reference/) --- подробная документация по языку.
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/index.html) --- руководство по написанию `unsafe` кода, [перевод](https://github.com/rust-lang-ru/rustonomicon/blob/master/src/SUMMARY.md).
- [Unsafe Code Guidelines Reference](https://rust-lang.github.io/unsafe-code-guidelines/) --- справочник по написанию `unsafe` кода.
- [The Embedded Rust Book](https://doc.rust-lang.org/stable/embedded-book/) --- руководство по написанию `no_std` кода.
- Справочник по [стандартной библиотеке `std`](https://doc.rust-lang.org/nightly/std/index.html) и её частям [`core`](https://doc.rust-lang.org/nightly/core/index.html) и [`alloc`](https://doc.rust-lang.org/nightly/alloc/index.html).
- Библиотека [tracing](https://docs.rs/tracing/).
- [Упражнения](https://github.com/rust-lang/rustlings/).
