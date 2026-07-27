## Отображение всей физической памяти в виртуальную

В этой задаче мы реализуем отображение всей физической памяти в виртуальную,
которое понадобится нам в дальнейшем для работы с
деревом отображения виртуальных страниц в физические фреймы.
Для построения этого отображения у нас будет только
[рекурсивная запись](https://os.phil-opp.com/page-tables/#recursive-mapping).
Но её достаточно, чтобы модифицировать всё дерево отображения.
В том числе, её достаточно для того,
чтобы построить линейное отображение всей физической памяти в виртуальную.
За это отвечает структура
[`kernel::memory::phys2virt::Phys2Virt`](../../doc/kernel/memory/phys2virt/struct.Phys2Virt.html).


### Задача 1 --- [`Phys2Virt::make()`](../../doc/kernel/memory/phys2virt/struct.Phys2Virt.html#method.make)

Напишите реализацию [метода](../../doc/kernel/memory/phys2virt/struct.Phys2Virt.html#method.make)

```rust
{{#include ../../kernel/src/memory/phys2virt.rs:make}}
```

Вы можете придерживаться структуры кода референсного решения, используя вспомогательные методы:

- [`Phys2Virt::find_empty_root_entries()`](../../doc/kernel/memory/phys2virt/struct.Phys2Virt.html#method.find_empty_root_entries),
- [`Phys2Virt::fill_entries()`](../../doc/kernel/memory/phys2virt/struct.Phys2Virt.html#method.fill_entries),
- [`Phys2Virt::allocate_root_entry()`](../../doc/kernel/memory/phys2virt/struct.Phys2Virt.html#method.allocate_root_entry),
- [`Phys2Virt::node()`](../../doc/kernel/memory/phys2virt/struct.Phys2Virt.html#method.node).

Или сделать всё по-своему.

Вам могут пригодиться методы:
- [`usize::div_ceil()`](https://doc.rust-lang.org/nightly/core/primitive.usize.html#method.div_ceil),
- [`MaybeUninit::fill()`](https://doc.rust-lang.org/nightly/core/mem/union.MaybeUninit.html#method.fill),
- [`Virt::canonize()`](../../doc/ku/memory/addr/type.Virt.html#method.canonize).


### Проверьте себя

Теперь должны заработать тесты второй лабораторной работы
с указанием дополнительно параметра `--no-default-features`.
При этом отключается построение отображения `phys2virt` загрузчиком
[`bootloader`](../../doc/bootloader/index.html)
и запускается
[`Phys2Virt::make()`](../../doc/kernel/memory/phys2virt/struct.Phys2Virt.html#method.make).

В частности, должен работать `2-mm-3-translate` с указанием `--no-default-features`:

```console
$ (cd kernel; cargo test --no-default-features --test 2-mm-3-translate)
...
2_mm_3_translate::t00_path----------------------------------
20:23:12 0 D present_virt = 0v100_0020_0DAC; path = L3: 0vFFFF_F080_0000_1010 / 1 @ 0p1000 -> L2: 0vFFFF_F080_0071_E000 / 1822 @ 0p71_E000 -> L1: 0vFFFF_F080_0071_F008 / 1823 @ 0p71_F000 -> L0: 0vFFFF_F080_0072_6000 / 1830 @ 0p72_6000 => 0p27_DDAC; level = 0
20:23:12 0 D pte = 637 @ 0p27_D000 0--DA---WPX(0x63)
20:23:12 0 D allocated unique virtual address; virt = 0v1080_0000_0000; is_user = true
20:23:12 0 D non_present_virt = 0v1080_0000_0000; path = L3: 0vFFFF_F080_0000_1108 / 1 @ 0p1000 (non-present); level = 3; pte = <non-present>
20:23:12 0 D huge_virt = 0vFFFF_F080_0000_0000; path = L3: 0vFFFF_F080_0000_1F08 / 1 @ 0p1000 -> L2: 0vFFFF_F080_07FD_F000 / 32735 @ 0p7FD_F000 (huge) => 0p0; level = 2; pte = 0 @ 0p0 0-HDA---WPX(0xE3)
2_mm_3_translate::t00_path------------------------- [passed]

2_mm_3_translate::t01_translate-----------------------------
20:23:12 0 D pte = 638 @ 0p27_E000 0--DA---WPX(0x63)
20:23:12 0 D read_ptr = 0xfffff0800027e270; write_ptr = 0x10000201270
20:23:12 0 D write_value = 0; read_value = 0; variable = 0
20:23:12 0 D write_value = 1; read_value = 1; variable = 1
20:23:12 0 D write_value = 2; read_value = 2; variable = 2
20:23:12 0 D write_value = 3; read_value = 3; variable = 3
20:23:12 0 D write_value = 4; read_value = 4; variable = 4
2_mm_3_translate::t01_translate-------------------- [passed]

2_mm_3_translate::t02_map_intermediate----------------------
20:23:13 0 D allocated unique virtual address; virt = 0v1100_0000_0000; is_user = true
20:23:13 0 D pte = <non-present>
20:23:13 0 D allocated unique virtual address; virt = 0v1180_0000_0000; is_user = true
2_mm_3_translate::t02_map_intermediate------------- [passed]

2_mm_3_translate::t03_no_excessive_intermediate_flags-------
20:23:13 0 D allocated unique virtual address; virt = 0v1200_0000_0000; is_user = true
20:23:13 0 D pte = ()
2_mm_3_translate::t03_no_excessive_intermediate_flags [passed]

2_mm_3_translate::t04_huge_page-----------------------------
2_mm_3_translate::t04_huge_page-------------------- [passed]

2_mm_3_translate::t05_no_page-------------------------------
20:23:13 0 D allocated unique virtual address; virt = 0v1280_0000_0000; is_user = true
2_mm_3_translate::t05_no_page---------------------- [passed]

2_mm_3_translate::t06_build_map-----------------------------
20:23:13 0 D virtual to physical mapping
20:23:13 0 D block = [0v1000, 0v1_6000), size 84.000 KiB -> [0p1000, 0p1_6000), size 84.000 KiB, 0-------WPX(0x3)
20:23:13 0 D block = [0vA_0000, 0vC_0000), size 128.000 KiB -> [0pA_0000, 0pC_0000), size 128.000 KiB, 0-------WPX(0x3)
20:23:13 0 D block = [0v20_0000, 0v21_8000), size 96.000 KiB -> [0p40_0000, 0p41_8000), size 96.000 KiB, 0--------P-(0x8000000000000001)
20:23:13 0 D block = [0v21_8000, 0v33_7000), size 1.121 MiB -> [0p41_7000, 0p53_6000), size 1.121 MiB, 0--------PX(0x1)
20:23:13 0 D block = [0v33_7000, 0v35_1000), size 104.000 KiB -> [0p53_5000, 0p54_F000), size 104.000 KiB, 0-------WP-(0x8000000000000003)
20:23:13 0 D block = [0v35_1000, 0v35_2000), size 4.000 KiB -> [0p1_7000, 0p1_8000), size 4.000 KiB, 0-------WP-(0x8000000000000003)
20:23:13 0 D block = [0v35_2000, 0v45_6000), size 1.016 MiB -> [0p54_F000, 0p65_3000), size 1.016 MiB, 0-------WP-(0x8000000000000003)
20:23:13 0 D block = [0v45_6000, 0v45_C000), size 24.000 KiB -> [0p1_8000, 0p1_E000), size 24.000 KiB, 0-------WP-(0x8000000000000003)
20:23:13 0 D block = [0v100_0000_0000, 0v100_0000_1000), size 4.000 KiB -> [0p1_6000, 0p1_7000), size 4.000 KiB, 0-------WPX(0x3)
20:23:14.023 0 D block = [0v100_0000_2000, 0v100_0008_3000), size 516.000 KiB -> [0p1_E000, 0p9_F000), size 516.000 KiB, 0-------WPX(0x3)
20:23:14.201 0 D block = [0v100_0008_3000, 0v100_0020_2000), size 1.496 MiB -> [0p10_0000, 0p27_F000), size 1.496 MiB, 0-------WPX(0x3)
20:23:14.557 0 D block = [0vFFFF_F000_0000_0000, 0vFFFF_F001_0020_0000), size 4.002 GiB -> [0p0, 0p1_0020_0000), size 4.002 GiB, 0-H-----WPX(0x83)
20:23:14.593 0 D block = [0vFFFF_F080_0000_0000, 0vFFFF_F080_4000_0000), size 1.000 GiB -> [0p0, 0p4000_0000), size 1.000 GiB, 0-H-----WPX(0x83)
20:23:14.603 0 D virtual address space
20:23:14.621 0 D block = [0v1000, 0v1_6000), size 84.000 KiB, 0-------WPX(0x3)
20:23:14.655 0 D block = [0vA_0000, 0vC_0000), size 128.000 KiB, 0-------WPX(0x3)
20:23:14.667 0 D block = [0v20_0000, 0v21_8000), size 96.000 KiB, 0--------P-(0x8000000000000001)
20:23:14.715 0 D block = [0v21_8000, 0v33_7000), size 1.121 MiB, 0--------PX(0x1)
20:23:14.853 0 D block = [0v33_7000, 0v45_C000), size 1.145 MiB, 0-------WP-(0x8000000000000003)
20:23:14.863 0 D block = [0v100_0000_0000, 0v100_0000_1000), size 4.000 KiB, 0-------WPX(0x3)
20:23:15.051 0 D block = [0v100_0000_2000, 0v100_0020_2000), size 2.000 MiB, 0-------WPX(0x3)
20:23:15.399 0 D block = [0vFFFF_F000_0000_0000, 0vFFFF_F001_0020_0000), size 4.002 GiB, 0-H-----WPX(0x83)
20:23:15.435 0 D block = [0vFFFF_F080_0000_0000, 0vFFFF_F080_4000_0000), size 1.000 GiB, 0-H-----WPX(0x83)
2_mm_3_translate::t06_build_map-------------------- [passed]

2_mm_3_translate::t07_duplicate_drop------------------------
20:23:15.649 0 D allocated unique virtual address; virt = 0v1300_0000_0000; is_user = true
20:23:15.659 0 D allocated unique virtual address; virt = 0v1380_0000_0000; is_user = true
20:23:16.031 0 I duplicate; address_space = "process" @ 0p7FD_2000
20:23:16.037 0 I drop; address_space = "process" @ 0p7FD_2000
20:23:16.831 0 I duplicate; address_space = "process" @ 0p7FB_E000
20:23:16.839 0 I drop; address_space = "process" @ 0p7FB_E000
2_mm_3_translate::t07_duplicate_drop--------------- [passed]

2_mm_3_translate::t08_duplicate_path------------------------
20:23:17.807 0 I duplicate; address_space = "process" @ 0p7FA_7000
20:23:17.813 0 I switch to; address_space = "process" @ 0p7FA_7000
20:23:17.819 0 D present_virt = 0v100_0020_0CAC; path = L3: 0vFFFF_F080_07FA_7010 / 32679 @ 0p7FA_7000 -> L2: 0vFFFF_F080_07FA_1000 / 32673 @ 0p7FA_1000 -> L1: 0vFFFF_F080_07FA_0008 / 32672 @ 0p7FA_0000 -> L0: 0vFFFF_F080_07F9_E000 / 32670 @ 0p7F9_E000 => 0p27_DCAC; level = 0
20:23:17.835 0 D pte = 637 @ 0p27_D000 0--DA---WPX(0x63)
20:23:17.841 0 D allocated unique virtual address; virt = 0v1400_0000_0000; is_user = true
20:23:17.849 0 D non_present_virt = 0v1400_0000_0000; path = L3: 0vFFFF_F080_07FA_7140 / 32679 @ 0p7FA_7000 (non-present); level = 3; pte = <non-present>
20:23:17.861 0 D huge_virt = 0vFFFF_F080_0000_0000; path = L3: 0vFFFF_F080_07FA_7F08 / 32679 @ 0p7FA_7000 -> L2: 0vFFFF_F080_07F9_7000 / 32663 @ 0p7F9_7000 (huge) => 0p0; level = 2; pte = 0 @ 0p0 0-HDA---WPX(0xE3)
20:23:17.875 0 I switch to; address_space = "base" @ 0p1000
20:23:17.881 0 I drop the current address space; address_space = "process" @ 0p7FA_7000; switch_to = "base" @ 0p1000
2_mm_3_translate::t08_duplicate_path--------------- [passed]

2_mm_3_translate::t09_duplicate_translate-------------------
20:23:18.795 0 I duplicate; address_space = "process" @ 0p7F9_6000
20:23:18.799 0 I switch to; address_space = "process" @ 0p7F9_6000
20:23:18.805 0 D pte = 638 @ 0p27_E000 0--DA---WPX(0x63)
20:23:18.811 0 D read_ptr = 0xfffff0800027e170; write_ptr = 0x10000201170
20:23:18.817 0 D write_value = 0; read_value = 0; variable = 0
20:23:18.823 0 D write_value = 1; read_value = 1; variable = 1
20:23:18.829 0 D write_value = 2; read_value = 2; variable = 2
20:23:18.835 0 D write_value = 3; read_value = 3; variable = 3
20:23:18.841 0 D write_value = 4; read_value = 4; variable = 4
20:23:18.849 0 I switch to; address_space = "base" @ 0p1000
20:23:18.853 0 I drop the current address space; address_space = "process" @ 0p7F9_6000; switch_to = "base" @ 0p1000
2_mm_3_translate::t09_duplicate_translate---------- [passed]

2_mm_3_translate::t10_drop_subtree--------------------------
20:23:19.417 0 D allocated unique virtual address; virt = 0v1480_0000_0000; is_user = true
20:23:19.429 0 D pte = <non-present>
20:23:19.745 0 D allocated unique virtual address; virt = 0v1500_0000_0000; is_user = true
20:23:19.757 0 D pte = <non-present>
20:23:19.975 0 D allocated unique virtual address; virt = 0v1580_0000_0000; is_user = true
20:23:19.987 0 D pte = ()
2_mm_3_translate::t10_drop_subtree----------------- [passed]

2_mm_3_translate::t11_no_frame------------------------------
20:23:24.805 0 D real free frame count can be less than free frame count reported by boot frame allocator since it records reference and deallocation counts but does not do that actually; frame_count = 30901; real_frame_count = 30800
20:23:24.817 0 D allocated unique virtual address; virt = 0v1600_0000_0000; is_user = true
2_mm_3_translate::t11_no_frame--------------------- [passed]
20:23:24.829 0 I exit qemu; exit_code = ExitCode(SUCCESS)
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/memory/phys2virt.rs | 121 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++-------
 1 file changed, 111 insertions(+), 10 deletions(-)
```
