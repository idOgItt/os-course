## Вытеснение блоков из блочного кеша


### Задача 6 --- LRU

Реализуйте политику вытеснения давно неиспользуемых данных ---
[Least Recently Used (LRU)](https://en.wikipedia.org/wiki/Cache_replacement_policies#LRU).

Её код находится в файле
[`ku/src/lru.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/src/lru.rs).

Вы можете следовать предложенному скелету реализации или же сделать всё по-своему.

Добавьте вытеснение блоков в вашу реализацию
[`BlockCache::trap_handler()`](../../doc/kernel/fs/block_cache/struct.BlockCache.html#method.trap_handler).


### Проверьте себя

Теперь должен заработать тесты в файле
[`ku/tests/6-fs-6-lru.rs`](https://gitlab.com/sergey-v-galtsev/nikka-public/-/blob/master/ku/tests/6-fs-6-lru.rs):

```console
$ (cd ku; cargo test --test 6-fs-6-lru)
...
running 3 tests
2023-12-03T17:40:32.893231Z : basic_lru lru=head: Some(0), tail: Some(4), lru: [{id: 0, key: 0, value: 0, prev: None, next: Some(1)}, {id: 1, key: 1, value: 2, prev: Some(0), next: Some(2)}, {id: 2, key: 2, value: 4, prev: Some(1), next: Some(3)}, {id: 3, key: 3, value: 6, prev: Some(2), next: Some(4)}, {id: 4, key: 4, value: 8, prev: Some(3), next: None}], map: {0 -> 0 [0], 1 -> 2 [1], 2 -> 4 [2], 3 -> 6 [3], 4 -> 8 [4]}
2023-12-03T17:40:32.893233Z : basic_map lru=head: Some(0), tail: Some(4), lru: [{id: 0, key: 0, value: 0, prev: None, next: Some(1)}, {id: 1, key: 1, value: 2, prev: Some(0), next: Some(2)}, {id: 2, key: 2, value: 4, prev: Some(1), next: Some(3)}, {id: 3, key: 3, value: 6, prev: Some(2), next: Some(4)}, {id: 4, key: 4, value: 8, prev: Some(3), next: None}], map: {0 -> 0 [0], 1 -> 2 [1], 2 -> 4 [2], 3 -> 6 [3], 4 -> 8 [4]}
2023-12-03T17:40:32.893489Z : basic_lru lru=head: Some(1), tail: Some(4), lru: [{id: 1, key: 5, value: 10, prev: None, next: Some(2)}, {id: 2, key: 6, value: 12, prev: Some(1), next: Some(3)}, {id: 3, key: 7, value: 14, prev: Some(2), next: Some(0)}, {id: 0, key: 8, value: 16, prev: Some(3), next: Some(4)}, {id: 4, key: 9, value: 18, prev: Some(0), next: None}], map: {5 -> 10 [1], 6 -> 12 [2], 7 -> 14 [3], 8 -> 16 [0], 9 -> 18 [4]}
2023-12-03T17:40:32.893507Z : basic_map lru=head: Some(1), tail: Some(4), lru: [{id: 1, key: 0, value: 0, prev: None, next: Some(2)}, {id: 2, key: 1, value: 3, prev: Some(1), next: Some(3)}, {id: 3, key: 2, value: 6, prev: Some(2), next: Some(0)}, {id: 0, key: 3, value: 9, prev: Some(3), next: Some(4)}, {id: 4, key: 4, value: 12, prev: Some(0), next: None}], map: {0 -> 0 [1], 1 -> 3 [2], 2 -> 6 [3], 3 -> 9 [0], 4 -> 12 [4]}
2023-12-03T17:40:32.893572Z : basic_lru lru=head: Some(0), tail: Some(4), lru: [{id: 0, key: 9, value: 27, prev: None, next: Some(3)}, {id: 3, key: 8, value: 24, prev: Some(0), next: Some(2)}, {id: 2, key: 7, value: 21, prev: Some(3), next: Some(1)}, {id: 1, key: 6, value: 18, prev: Some(2), next: Some(4)}, {id: 4, key: 5, value: 15, prev: Some(1), next: None}], map: {5 -> 15 [4], 6 -> 18 [1], 7 -> 21 [2], 8 -> 24 [3], 9 -> 27 [0]}
2023-12-03T17:40:32.893634Z : basic_lru lru=head: Some(3), tail: Some(4), lru: [{id: 3, key: 14, value: 28, prev: None, next: Some(2)}, {id: 2, key: 13, value: 26, prev: Some(3), next: Some(1)}, {id: 1, key: 12, value: 24, prev: Some(2), next: Some(0)}, {id: 0, key: 11, value: 22, prev: Some(1), next: Some(4)}, {id: 4, key: 10, value: 20, prev: Some(0), next: None}], map: {10 -> 20 [4], 11 -> 22 [0], 12 -> 24 [1], 13 -> 26 [2], 14 -> 28 [3]}
test basic_map ... ok
test basic_lru ... ok
test stress ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.54s
```


### Ориентировочный объём работ этой части лабораторки

```console
 kernel/src/fs/block_cache.rs |  13 ++++++++++-
 ku/src/lru.rs                | 103 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++------------
 2 files changed, 101 insertions(+), 15 deletions(-)
```
