use core::sync::atomic::Ordering;

use lazy_static::lazy_static;
use spin::Mutex;

use ku::{
    error::{Error::NoDisk, Result},
    log::trace,
    lru::Lru,
    memory::{
        mmu::{PageTableFlags, KERNEL_RW},
        Block,
        Page,
        PageFaultInfo,
        Virt,
    },
    process::Info,
};

use crate::memory::{mmu, BASE_ADDRESS_SPACE};

use super::{
    disk::{Disk, SECTOR_SIZE},
    BLOCK_SIZE,
};


// ANCHOR: block_cache
/// [Блочный кеш](https://en.wikipedia.org/wiki/Page_cache)
/// для ускорения работы с диском за счёт кеширования блоков файловой системы в памяти.
#[derive(Clone, Debug)]
pub struct BlockCache {
    /// Диапазон памяти для кеширования блоков.
    cache: Block<Page>,

    /// Диск, обращения к которому кешируются.
    disk: Disk,

    /// Политика вытеснения блоков из кеша.
    eviction_policy: Lru<usize, ()>,

    /// Статистика работы блочного кеша.
    stats: Stats,
}
// ANCHOR_END: block_cache


impl BlockCache {
    // ANCHOR: init
    /// Инициализирует блочный кеш в
    /// [синглтоне](https://en.wikipedia.org/wiki/Singleton_pattern)
    /// [`struct@BLOCK_CACHE`].
    ///
    /// Резервирует в [`BASE_ADDRESS_SPACE`] блок виртуальных страниц,
    /// достаточный для отображения 1-в-1 `block_count` блоков файловой системы.
    /// Политика вытеснения блоков из кеша ограничивает
    /// количество одновременно отображённых в память блоков параметром `capacity`.
    pub(super) fn init(disk: Disk, block_count: usize, capacity: usize) -> Result<()> {
        // ANCHOR_END: init
        // TODO: your code here.
        unimplemented!();
    }


    /// Возвращает блок памяти блочного кеша [`struct@BLOCK_CACHE`],
    /// который отвечает блоку `block_number` диска.
    pub(super) fn block(block_number: usize) -> Result<Block<Virt>> {
        if let Some(block_cache) = BLOCK_CACHE.lock().as_mut() {
            block_cache.block_impl(block_number)
        } else {
            Err(NoDisk)
        }
    }


    /// Записывает блок `block_number` на диск.
    ///
    /// См. также [`BlockCache::flush_block_impl()`].
    pub(super) fn flush_block(block_number: usize) -> Result<()> {
        if !test_scaffolding::FLUSH_ENABLED.load(Ordering::Relaxed) {
            return Ok(());
        }

        if let Some(block_cache) = BLOCK_CACHE.lock().as_mut() {
            block_cache.flush_block_impl(block_number)?;
            block_cache.disk.flush()
        } else {
            Err(NoDisk)
        }
    }


    /// Сбрасывает первые `count` блоков на диск.
    ///
    /// См. также [`BlockCache::flush_block_impl()`].
    pub(super) fn flush(count: usize) -> Result<()> {
        if let Some(block_cache) = BLOCK_CACHE.lock().as_mut() {
            for block_number in 0..count {
                block_cache.flush_block_impl(block_number)?;
            }

            block_cache.disk.flush()
        } else {
            Err(NoDisk)
        }
    }


    // ANCHOR: trap_handler
    /// Обрабатывает Page Fault, если адрес, который его вызвал, относится к блочному кешу.
    /// Если это так и Page Fault успешно обработан, возвращает `true`.
    /// Если адрес, вызвавший Page Fault, не относится к блочному кешу, возвращает `false`.
    pub(crate) fn trap_handler(info: &Info) -> Result<bool> {
        // ANCHOR_END: trap_handler
        // TODO: your code here.
        Ok(false) // TODO: remove before flight.
    }


    /// Статистика работы блочного кеша.
    pub fn stats() -> Stats {
        if let Some(block_cache) = BLOCK_CACHE.lock().as_ref() {
            block_cache.stats
        } else {
            Stats::default()
        }
    }


    // ANCHOR: block_impl
    /// Возвращает блок памяти блочного кеша,
    /// который отвечает блоку `block_number` диска.
    pub(super) fn block_impl(&self, block_number: usize) -> Result<Block<Virt>> {
        // ANCHOR_END: block_impl
        // TODO: your code here.
        unimplemented!();
    }


    // ANCHOR: flush_block_impl
    /// Записывает блок `block_number` на диск, если:
    ///
    /// - Блок отображён в память. Это означает, что к нему были обращения.
    /// - И помечен как [`PageTableFlags::DIRTY`].
    ///   То есть, в память были записи, а значит блок на диске потенциально содержит
    ///   устаревшие данные.
    ///   Если обращения к блоку были только на чтение, то данные в памяти такие же как на диске,
    ///   и можно их не записывать.
    ///   А процессор в этом случае не установит бит [`PageTableFlags::DIRTY`].
    ///
    /// После записи блока, сбрасывает бит [`PageTableFlags::DIRTY`].
    /// Он фактически означает одинаковость данных на диске и в памяти блочного кеша.
    /// Которая только что восстановлена.
    /// При этом сбрасывает и соответствующую запись в
    /// [TLB](https://en.wikipedia.org/wiki/Translation_lookaside_buffer)
    /// с помощью функции [`mmu::flush()`].
    /// Иначе процессор не узнает, что сброшен [`PageTableFlags::DIRTY`]
    /// и не проставит его в таблице страниц при следующей записи.
    /// В результате, обновлённый блок на диск записан не будет.
    fn flush_block_impl(&mut self, block_number: usize) -> Result<()> {
        // ANCHOR_END: flush_block_impl
        // TODO: your code here.
        unimplemented!();
    }
}


impl Drop for BlockCache {
    fn drop(&mut self) {
        let block_count = self.cache.count() * Page::SIZE / BLOCK_SIZE;

        for block_number in 0..block_count {
            self.flush_block_impl(block_number).expect("failed to flush the block cache");
        }
    }
}


/// Статистика работы блочного кеша.
#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    /// Количество блоков, которые не пришлось записывать на диск в [`BlockCache::flush_block_impl()`].
    discards: usize,

    /// Количество блоков, которые были вытеснены из кеша в [`BlockCache::trap_handler()`].
    evictions: usize,

    /// Количество блоков, которые были прочитаны с диска в [`BlockCache::trap_handler()`].
    reads: usize,

    /// Количество блоков, которые были записаны на диск в [`BlockCache::flush_block_impl()`].
    writes: usize,
}


lazy_static! {
    /// Блочный кеш для ускорения работы с диском
    /// за счёт кеширования блоков файловой системы в памяти.
    pub(super) static ref BLOCK_CACHE: Mutex<Option<BlockCache>> = Mutex::new(None);
}


/// Количество секторов диска в одном блоке файловой системы.
pub(super) const SECTORS_PER_BLOCK: usize = BLOCK_SIZE / SECTOR_SIZE;


#[doc(hidden)]
pub mod test_scaffolding {
    use core::sync::atomic::{AtomicBool, Ordering};

    use ku::{
        error::{Error::NoDisk, Result},
        memory::{Block, Page},
    };

    use super::{BlockCache, Disk, BLOCK_CACHE};


    pub fn block_cache_init(disk: usize, block_count: usize, capacity: usize) -> Result<()> {
        BlockCache::init(Disk::new(disk)?, block_count, capacity)
    }


    pub fn cache() -> Result<Block<Page>> {
        Ok(BLOCK_CACHE.lock().as_ref().ok_or(NoDisk)?.cache)
    }


    pub fn flush_block(block_number: usize) -> Result<()> {
        BlockCache::flush_block(block_number)
    }


    pub fn disable_flush() {
        FLUSH_ENABLED.store(false, Ordering::Relaxed);
    }


    pub(super) static FLUSH_ENABLED: AtomicBool = AtomicBool::new(true);
}
