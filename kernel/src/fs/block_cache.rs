use core::sync::atomic::Ordering;

use lazy_static::lazy_static;
use spin::Mutex;

use ku::{
    error::{Error::NoDisk, Result},
    log::trace,
    lru::Lru,
    memory::{
        block::Memory,
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
        let mut address_space = BASE_ADDRESS_SPACE.lock();

        let page_count = block_count * BLOCK_SIZE;

        let cache = address_space.allocate(page_count, KERNEL_RW)?;
        drop(address_space);

        let eviction_policy = Lru::new(capacity);
        let block_cache = BlockCache {
            cache,
            disk,
            eviction_policy,
            stats: Stats::default(),
        };

        *BLOCK_CACHE.lock() = Some(block_cache);

        Ok(())
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
        if let Info::PageFault { address, code: _ } = info {
            let fault_addr = *address;

            let mut block_cache_guard = BLOCK_CACHE.lock();
            if let Some(block_cache) = block_cache_guard.as_mut() {
                if block_cache.cache.contains_address(fault_addr) {
                    let block_number = (Page::containing(fault_addr) -
                        Page::containing(block_cache.cache.start_address()))
                    .unwrap();

                    let virt_block = block_cache.block_impl(block_number)?;

                    let mut addr_space = BASE_ADDRESS_SPACE.lock();
                    unsafe { addr_space.map_block(virt_block.enclosing(), KERNEL_RW) }?;
                    drop(addr_space);

                    let buffer = unsafe { virt_block.try_into_mut_slice::<u32>()? };

                    block_cache.disk.pio_read(
                        block_number * SECTORS_PER_BLOCK..(block_number + 1) * SECTORS_PER_BLOCK,
                        buffer,
                    )?;

                    block_cache.stats.reads += 1;

                    return Ok(true);
                }
            }
        }

        Ok(false)
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
        let block = self.cache.slice(block_number..block_number + 1).unwrap();
        let virt_block = block.into();

        Ok(virt_block)
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
        let virt_block = self.block_impl(block_number)?;
        let virt_addr = virt_block.start_address();

        let mut addr_space = BASE_ADDRESS_SPACE.lock();

        let pte = addr_space.mapping().translate(virt_addr)?;

        if pte.flags().contains(PageTableFlags::DIRTY) {
            let buffer = unsafe { virt_block.try_into_mut_slice::<u32>()? };

            let block = Page::containing(virt_addr);

            self.disk.pio_write(
                block_number * SECTORS_PER_BLOCK..(block_number + 1) * SECTORS_PER_BLOCK,
                buffer,
            )?;

            unsafe { mmu::flush(block) };

            self.stats.writes += 1;
        } else {
            self.stats.discards += 1;
        }

        Ok(())
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
