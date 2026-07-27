# Summary

[Введение в лабораторные работы](0-intro-1-nikka.md)
[Почему Rust](0-intro-2-rust.md)
[Структура кода](0-intro-3-dirs.md)
[Компиляция и запуск тестов](0-intro-4-install.md)
[Отладчик, дизассемблирование, бектрейсы](0-intro-5-gdb.md)
[Логирование и логи](0-intro-6-log.md)
[Настройка VSCode](0-intro-7-vscode.md)
[Ссылки](0-intro-8-links.md)

# Лабораторная работа №1

- [Время, прерывания и конкурентный код](1-time-0-intro.md)
  - [Часы реального времени](1-time-1-rtc.md)
  - [Счётчик тактов процессора](1-time-2-tsc.md)
  - [Блокировки](1-time-3-locks.md)
    - [Once lock](1-time-3-locks-1-once-lock.md)
    - [Спин–блокировка](1-time-3-locks-2-spinlock.md)
    - [Sequence lock](1-time-3-locks-3-sequence-lock.md)
  - [Счётчики тиков](1-time-4-correlation-point.md)
  - [Измерение частоты процессора и повышение разрешения часов](1-time-5-correlation-interval.md)
  - [Обработка прерываний RTC](1-time-6-interrupts.md)
  - [Информация о системе](1-time-7-info.md)
  - [Собираем всё вместе](1-time-8-summary.md)

# Лабораторная работа №2

- [Память](2-mm-0-intro.md)
  - [Типы](2-mm-1-types.md)
  - [Страничные отображения](2-mm-2-mapping.md)
  - [Диаграммы преобразований](2-mm-3-diagrams.md)
  - [План](2-mm-4-plan.md)
  - [Временный аллокатор фреймов](2-mm-5-boot-frame-allocator.md)
  - [Виртуальное адресное пространство](2-mm-6-address-space.md)
    - [Аллокатор виртуальных страниц адресного пространства](2-mm-6-address-space-1-allocation.md)
    - [Отображение всей физической памяти в виртуальную](2-mm-6-address-space-2-phys2virt.md)
    - [Отображение виртуальных страниц в физические фреймы](2-mm-6-address-space-2-translate.md)
    - [Высокоуровневый интерфейс управления адресным пространством](2-mm-6-address-space-3-map.md)
  - [Основной аллокатор фреймов](2-mm-7-main-frame-allocator.md)
  - [Проверьте себя](2-mm-8-tests.md)

# Лабораторная работа №3

- [Аллокатор памяти общего назначения и SMP](3-smp-0-intro.md)
  - [Аллокатор памяти общего назначения](3-smp-1-memory-allocator-0-traits.md)
    - [Аллокатор больших блоков памяти](3-smp-1-memory-allocator-1-big.md)
    - [Аллокатор маленьких блоков памяти](3-smp-1-memory-allocator-2-small.md)
  - [Поддержка нескольких процессоров (SMP)](3-smp-2-smp-0-intro.md)
    - [Работа с local APIC](3-smp-2-smp-1-local-apic.md)
    - [Состояние каждого процессора](3-smp-2-smp-2-cpus.md)
    - [Загрузка Application Processor](3-smp-2-smp-3-ap-init.md)

# Лабораторная работа №4

- [Процессы](4-process-0-intro.md)
  - [Загрузка процесса в память](4-process-1-elf.md)
  - [Проверки доступа процесса к памяти](4-process-2-permission-checks.md)
  - [Переход в режим пользователя](4-process-3-user-mode.md)
  - [Поддержка системных вызовов](4-process-4-syscall.md)
  - [Таблица процессов](4-process-5-table.md)
  - [Вытесняющая многозадачность](4-process-6-preemption.md)
  - [Планировщик](4-process-7-scheduler.md)

# Лабораторная работа №5

- [Продвинутая работа с памятью в пространстве пользователя](5-um-0-intro.md)
  - [Разделяемая память](5-um-1-pipe.md)
  - [Системные вызовы для работы с виртуальной памятью](5-um-2-memory.md)
  - [Eager fork](5-um-3-eager-fork.md)
  - [Обработка исключений в режиме пользователя](5-um-4-trap-handler.md)
  - [Copy-on-write fork](5-um-5-cow-fork.md)

# Лабораторная работа №6

- [Файловая система](6-fs-0-intro.md)
  - [Блочный кеш](6-fs-1-block-cache.md)
  - [Битмапы занятых блоков и inode](6-fs-2-bitmap.md)
  - [Index node (inode)](6-fs-3-inode.md)
  - [Операции с директориями](6-fs-4-directory.md)
  - [Поиск файла по пути](6-fs-5-open.md)
  - [Вытеснение блоков из блочного кеша](6-fs-6-lru.md)
