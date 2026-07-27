use core::{
    mem,
    ops::{Add, Range},
};

use ku::{
    error::{
        Error::{Medium, NoDisk},
        Result,
    },
    memory::size,
};

use super::{block_cache::BlockCache, BLOCK_SIZE};

// Used in docs.
#[allow(unused)]
use ku::error::Error;


/// [Битмап](https://en.wikipedia.org/wiki/Free-space_bitmap)
/// для отслеживания какие именно элементы
/// (блоки или [inode](https://en.wikipedia.org/wiki/Inode) для соответствующего из битмапов)
/// [файловой системы](https://en.wikipedia.org/wiki/File_system) заняты, а какие свободны.
#[derive(Debug)]
pub(super) struct Bitmap {
    /// [Битмап](https://en.wikipedia.org/wiki/Free-space_bitmap),
    /// каждый элемент этого среза отвечает за [`Self::BITS_PER_ENTRY`] элементов.
    /// Сам срез хранится в памяти блочного кеша [`BlockCache`].
    bitmap: &'static mut [u64],

    /// Номер блока на диске, с которого записан сам битмап.
    block: usize,

    /// Диапазон элементов для пользовательских данных файловой системы.
    /// Полное количество элементов в [файловой системе](https://en.wikipedia.org/wiki/File_system),
    /// то есть количество используемых бит в битмапе, равно `Bitmap::elements.end`.
    /// Элементы с нулевого до начала этого диапазона (не включительно),
    /// то есть диапазон `0..Bitmap::elements.start`, зарезервированы.
    elements: Range<usize>,

    /// Место последней аллокации из битмапа.
    /// Служит для ускорения аллокаций и равномерного распределения занятых элементов.
    cursor: usize,
}


impl Bitmap {
    /// Возвращает битмап файловой системы или ошибку [`Error::Medium`],
    /// если отведённое под битмап место диска содержит данные,
    /// которые не похожи на корректный битмап.
    pub(super) fn new(block: usize, elements: Range<usize>) -> Result<Self> {
        let bitmap = Self::new_unchecked(block, elements);

        bitmap.validate()
    }


    /// Форматирует часть диска, начинающуюся с блока `block` и
    /// отведённую под битмап для `elements.end` элементов,
    /// его начальным состоянием.
    /// Биты для элементов вне диапазона [`Bitmap::elements`] выставляет в единицы.
    pub(super) fn format(block: usize, elements: Range<usize>) -> Result<()> {
        let bitmap = Self::new_unchecked(block, elements.clone());

        // TODO: your code here.
        unimplemented!();

        bitmap.validate()?;

        Ok(())
    }


    /// Возвращает `true`, если элемент `number` свободен.
    ///
    /// # Panics
    ///
    /// Паникует, если `number` выходит за пределы диска --- `Bitmap::elements.end`.
    pub(super) fn is_free(&self, number: usize) -> bool {
        assert!(number < self.elements.end);
        self.bitmap[number / Self::BITS_PER_ENTRY] & (1 << (number % Self::BITS_PER_ENTRY)) == 0
    }


    /// Помечает элемент `number` как свободный.
    ///
    /// # Panics
    ///
    /// Паникует, если:
    ///   - Значение `number` выходит за пределы файловой системы --- `Bitmap::block_count.end`.
    ///   - Элемент `number` зарезервирован.
    ///   - Элемент `number` уже помечен как свободный.
    pub(super) fn set_free(&mut self, number: usize) {
        assert!(self.elements.contains(&number));
        assert!(!self.is_free(number));
        self.bitmap[number / Self::BITS_PER_ENTRY] &= !(1 << (number % Self::BITS_PER_ENTRY));
    }


    /// Находит по [`Bitmap::bitmap`] свободный элемент и аллоцирует его.
    /// Возвращает номер выделенного элемента или
    /// ошибку [`Error::NoDisk`], если свободных элементов не осталось.
    ///
    /// При поиске свободного элемента стартует с номера [`Bitmap::cursor`] в
    /// срезе [`Bitmap::bitmap`] и запоминает в неё позицию,
    /// из которого выделен свободный элемент.
    ///
    /// Может опираться на то, что биты для элементов вне диапазона [`Bitmap::elements`]
    /// гарантированно выставлены в единицы, см. [`Bitmap::format()`].
    pub(super) fn allocate(&mut self) -> Result<usize> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Возвращает количество свободных элементов в файловой системе.
    pub(super) fn free_count(&self) -> usize {
        self.bitmap
            .iter()
            .map(|bits| bits.count_zeros().try_into().unwrap())
            .reduce(Add::add)
            .unwrap_or(0)
    }


    /// Возвращает количество блоков, которые занимает [`Bitmap`] на диске.
    pub(super) fn size_in_blocks(elements: usize) -> usize {
        elements.div_ceil(size::from(u8::BITS) * BLOCK_SIZE)
    }


    /// Создаёт [`Bitmap`], не проверяя корректность его данных на диске.
    fn new_unchecked(block: usize, elements: Range<usize>) -> Self {
        Self {
            bitmap: Self::bitmap(block, elements.clone()),
            block,
            elements,
            cursor: 0,
        }
    }


    /// Проверяет корректность данных [`Bitmap`] на диске.
    /// Возвращает ошибку [`Error::Medium`],
    /// если данные на диске заведомо не содержат корректный [`Bitmap`],
    /// или сам [`Bitmap`] иначе.
    fn validate(self) -> Result<Self> {
        let bit_count = self.bitmap.len() * Self::BITS_PER_ENTRY;
        if self.elements.start >= self.elements.end || bit_count < self.elements.end {
            return Err(Medium);
        }

        let unused_elements = self.elements.end != self.bitmap.len() * Self::BITS_PER_ENTRY;
        if unused_elements {
            let unused_elements_expected_bitmap = !0 << (self.elements.end % Self::BITS_PER_ENTRY);
            let unused_elements_actual_bitmap =
                self.bitmap[self.bitmap.len() - 1] & unused_elements_expected_bitmap;
            if unused_elements_actual_bitmap != unused_elements_expected_bitmap {
                return Err(Medium);
            }
        }

        if (0..self.elements.start).all(|reserved_element| !self.is_free(reserved_element)) {
            Ok(self)
        } else {
            Err(Medium)
        }
    }


    /// Возвращает срез элементов битмапа в памяти блочного кеша [`Bitmap`].
    fn bitmap(block: usize, elements: Range<usize>) -> &'static mut [u64] {
        unsafe {
            BlockCache::block(block)
                .unwrap()
                .start_address()
                .try_into_mut_slice::<u64>(elements.end.div_ceil(Self::BITS_PER_ENTRY))
                .unwrap()
        }
    }


    /// Количество элементов, за которые отвечает один элемент среза [`Bitmap::bitmap`].
    const BITS_PER_ENTRY: usize = u64::BITS as usize;
}


impl Clone for Bitmap {
    fn clone(&self) -> Self {
        Self {
            bitmap: Self::bitmap(self.block, self.elements.clone()),
            block: self.block,
            elements: self.elements.clone(),
            cursor: self.cursor,
        }
    }
}


#[doc(hidden)]
pub mod test_scaffolding {
    use core::ops::Range;

    use ku::error::Result;


    #[derive(Debug)]
    pub struct Bitmap(pub(in super::super) super::Bitmap);


    impl Bitmap {
        pub fn new(block: usize, elements: Range<usize>) -> Result<Self> {
            Ok(Self(super::Bitmap::new(block, elements)?))
        }


        pub fn format(block: usize, elements: Range<usize>) -> Result<()> {
            super::Bitmap::format(block, elements)
        }


        pub fn is_free(&self, number: usize) -> bool {
            self.0.is_free(number)
        }


        pub fn set_free(&mut self, number: usize) {
            self.0.set_free(number)
        }


        pub fn allocate(&mut self) -> Result<usize> {
            self.0.allocate()
        }


        pub fn free_count(&self) -> usize {
            self.0.free_count()
        }


        pub fn size_in_blocks(elements: usize) -> usize {
            super::Bitmap::size_in_blocks(elements)
        }
    }
}
