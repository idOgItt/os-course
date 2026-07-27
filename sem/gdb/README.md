# gdb

В директории задачи находится исполняемый файл `binary` (Убедитесь что он последней версии!).

Исходный код функции `main()` вам дан:

```
fn main() -> Result<(), Error> {
    let key = fs::read_to_string("key.txt")?;

    if check_key(&key.as_bytes()) {
        println!("OK");

        Ok(())
    } else {
        Err(Error::from(InvalidData))
    }
}
```

Исходный код функции `check_key()` вам неизвестен.
Но и писать её реализацию не требуется.
Хотя это тоже хорошее дополнительное упражнение.

Вам нужно понять, какой ключ нужно подать на вход исполняемому файлу,
чтобы он напечатал `OK`.

0. Запишите в файл `key.txt` вашу гипотезу.

```
echo -n 1234asdf > key.txt
```

1. Установите плагин `peda` к `gdb`.

```
git clone https://github.com/longld/peda.git ~/peda
echo "source ~/peda/peda.py" >> ~/.gdbinit
echo "set disassembly-flavor intel" >> ~/.gdbinit
echo "set print asm-demangle on" >> ~/.gdbinit
echo "DONE! debug your program with gdb and enjoy"
```

2. Запустите `gdb`.

```
gdb ./binary
```
Если у вас нет нужных динамических библиотек, можно анализировать статически собранный бинарник:
```
gdb ./binary.static
```

3. Поставьте `breakpoint` на функцию `check_key()` и запустите программу.

```
b check_key
run
```

Если gdb ломается с ошибкой использования `libthread_db`
```
gdb-peda$ run
Starting program: /home/sergey/nikka/sem/gdb/binary.static
[Thread debugging using libthread_db enabled]
Using host libthread_db library "/lib/x86_64-linux-gnu/libthread_db.so.1".
Cannot find user-level thread for LWP 192000: debugger service failed
```
можно изменить путь поиска этой библиотеки, чтобы `gdb` её не нашёл:
```
gdb-peda$ set libthread-db-search-path /
gdb-peda$ run
```

Также можно попробовать использовать `lldb` вместо `gdb`.

4. Пошагайте по инструкциям и попробуйте понять что происходит.

```
ni
```

Или если хотите заходить в вызываемые функции:

```
si
```

Быстро дойти до конца функции можно командой

```
finish
```
