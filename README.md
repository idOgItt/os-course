# Операционная система на Rust

Проект реализует компоненты x86_64-ядра и системные примитивы: управление памятью, многопроцессорный запуск, синхронизацию, файловый кэш и набор Unix-утилит.

## Что реализовано

- загрузка ядра, системные вызовы и переключение между kernel/user mode;
- boot/main/page allocators, виртуальная память и трансляция адресов;
- поддержка нескольких CPU, Local APIC, RTC и high-resolution timer;
- atomics, spinlock, RWLock, sequence lock и once lock;
- LRU и block cache;
- backtrace и разбор низкоуровневых механизмов ROP;
- remote shell с файлами, `fork` и pipes, а также jobserver.

## Как читать репозиторий

Решения хранятся в отдельных ветках `submit/*`, а `main` служит индексом:

- [`submit/boot-frame-allocator`](https://github.com/idOgItt/os-course/tree/submit/boot-frame-allocator) — ранний аллокатор физических фреймов;
- [`submit/local-apic`](https://github.com/idOgItt/os-course/tree/submit/local-apic) — многопроцессорный запуск и APIC;
- [`submit/block-cache`](https://github.com/idOgItt/os-course/tree/submit/block-cache) — кэш дисковых блоков;
- [`submit/rsh_pipe`](https://github.com/idOgItt/os-course/tree/submit/rsh_pipe) — shell и конвейеры;
- [`submit/jobserver`](https://github.com/idOgItt/os-course/tree/submit/jobserver) — координация параллельных процессов.

Стек: Rust, x86_64 Assembly, Cargo, QEMU, Make и Docker.
