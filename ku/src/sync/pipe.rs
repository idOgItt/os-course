#![allow(rustdoc::private_intra_doc_links)]


use core::{
    alloc::Layout,
    cmp,
    marker::PhantomData,
    mem,
    result,
    sync::atomic::{AtomicU8, Ordering},
};

use crate::{
    allocator::BigAllocator,
    error::{self, Error::WrongAlignment},
    log::{debug, error},
    memory::{size, Block, Page, Virt},
};


/// Создаёт однонаправленный канал для передачи последовательностей байт.
///
/// Отображает в память непрерывный циклический буфер [`RingBuffer`].
/// Использует для построения отображения `allocator`,
/// который должен реализовывать типаж постраничного аллокатора памяти
/// [`BigAllocator`].
///
/// Возвращает пару из [`ReadBuffer`] и [`WriteBuffer`] ---
/// интерфейса читателя и интерфейса писателя для этого буфера.
pub fn make<T: BigAllocator>(allocator: &mut T) -> error::Result<(ReadBuffer, WriteBuffer)> {
    // TODO: your code here.
    Ok((ReadBuffer::default(), WriteBuffer::default())) // TODO: remove before flight.
}


// ANCHOR: ring_buffer
/// [Непрерывный циклический буфер](https://fgiesen.wordpress.com/2012/07/21/the-magic-ring-buffer/).
#[derive(Debug, Default)]
pub struct RingBuffer<T: Tag> {
    /// Блок памяти с данными, которые хранятся в буфере.
    block: Block<Page>,

    /// Закрыт ли буфер.
    closed: bool,

    /// Количество байт, прочитанных из буфера за всё время.
    /// То есть, эта величина потенциально больше размера буфера.
    /// Вариант хранить в [`RingBuffer`] значения по модулю размера буфера, чреват ошибками.
    ///
    /// Аналогично, все методы работают с позициями в буфере,
    /// измеряя их от момента инициализации буфера.
    /// Единственное преобразование таких позиций в смещения в памяти, ---
    /// взятие по модулю `REAL_SIZE`, ---
    /// выполняется в реализации метода [`RingBuffer::get()`].
    head: usize,

    /// Количество байт, записанных в буфер за всё время.
    /// То есть, эта величина потенциально больше размера буфера.
    /// Вариант хранить в [`RingBuffer`] значения по модулю размера буфера, чреват ошибками.
    ///
    /// Аналогично, все методы работают с позициями в буфере,
    /// измеряя их от момента инициализации буфера.
    /// Единственное преобразование таких позиций в смещения в памяти, ---
    /// взятие по модулю `REAL_SIZE`, ---
    /// выполняется в реализации метода [`RingBuffer::get()`].
    tail: usize,

    /// Статистики чтения или записи в буфер.
    stats: RingBufferStats,

    /// Тег, отличающий писателя от читателя.
    _tag: PhantomData<T>,
}
// ANCHOR_END: ring_buffer


impl<T: Tag> RingBuffer<T> {
    /// Инициализирует буфер над блоком памяти `buf`.
    fn new(block: Block<Page>) -> Self {
        assert_ne!(block.start_address(), Virt::default());
        assert_eq!(block.size(), MAPPED_SIZE);

        Self {
            block,
            closed: false,
            head: 0,
            tail: 0,
            stats: RingBufferStats::default(),
            _tag: PhantomData,
        }
    }


    /// Возвращает блок виртуальной памяти, в которую отображён буфер.
    pub fn block(&self) -> Block<Page> {
        self.block
    }


    /// Закрывает [`RingBuffer`].
    /// После вызова этой функции любым из участников
    /// никакие транзакции больше создаваться не могут.
    /// А соответствующие методы [`ReadBuffer::read_tx()`] и [`WriteBuffer::write_tx()`]
    /// возвращают [`None`].
    ///
    /// Для упрощения, чтобы не реализовывать сложного протокола закрытия,
    /// этот метод работает ассимметрично.
    /// Если буфер закрывает пишущая сторона, то читающая сначала прочитает все данные,
    /// которые были записаны в буфер до закрытия.
    /// Если же буфер закроет читающая сторона, то все данные, которые в него были записаны или
    /// писались в этом момент конкурентно, будyт потеряны.
    pub fn close(&mut self) {
        // TODO: your code here.
        unimplemented!();
    }


    /// Читает заголовок записи, находящейся на позиции `position`.
    /// В случае, если заголовок записи некорректен,
    /// считает что противоположная сторона закрыла буфер и возвращает
    /// [`Header::Closed`].
    fn read_header(&mut self, position: usize) -> Header {
        // TODO: your code here.
        unimplemented!();
    }


    /// Записывает заголовок записи, находящейся на позиции `position`.
    fn write_header(&mut self, position: usize, header: Header) {
        // TODO: your code here.
        unimplemented!();
    }


    /// Читает размер из заголовка записи, находящейся на позиции `position`.
    fn read_size(&mut self, position: usize) -> usize {
        let mut size = 0;
        for x in self.size(position) {
            size = (size << u8::BITS) | size::from(*x);
        }
        size
    }


    /// Записывает размер `size` в заголовок записи, находящейся на позиции `position`.
    fn write_size(&mut self, position: usize, size: usize) {
        let mut size = size;
        for x in self.size(position).iter_mut().rev() {
            *x = size as u8;
            size >>= u8::BITS;
        }
        assert!(size == 0);
    }


    /// Возвращает ссылку на флаг состояния записи, находящейся на позиции `position`.
    ///
    /// Принимает `&mut self`, чтобы гарантировать отсутствие алиасинга.
    fn state(&mut self, position: usize) -> &AtomicU8 {
        AtomicU8::from_mut(self.get(position))
    }


    /// Возвращает ссылку на поле заголовка с размером записи,
    /// которая находится на позиции `position`.
    ///
    /// Принимает `&mut self`, чтобы гарантировать отсутствие алиасинга.
    fn size(&mut self, position: usize) -> &mut [u8; HEADER_SIZE - mem::size_of::<AtomicU8>()] {
        self.get(position + mem::size_of::<AtomicU8>())
    }


    /// Возвращает ссылку на буфер с данными, находящийся на позиции `position`.
    ///
    /// Принимает `&mut self`, чтобы гарантировать отсутствие алиасинга.
    fn buf(&mut self, position: usize) -> &mut [u8; REAL_SIZE] {
        self.get(position)
    }


    /// Возвращает ссылку на тип `Q`, записанный на позиции `position`.
    /// Позиция `position` измеряется от момента инициализации буфера,
    /// то есть не учитывает переполнения размера буфера.
    /// Тип `Q` должен влезать в буфер и требовать тривиального выравнивания.
    ///
    /// # Panics
    ///
    /// Паникует, если:
    ///   - тип `Q` не влезает в буфер;
    ///   - тип `Q` требует нетривиального выравнивания.
    ///
    /// Принимает `&mut self`, чтобы гарантировать отсутствие алиасинга.
    fn get<Q>(&mut self, position: usize) -> &mut Q {
        assert!(mem::size_of::<Q>() <= REAL_SIZE);

        let offset = position % REAL_SIZE;
        let block = Block::<Virt>::from(self.block)
            .slice(offset..offset + mem::size_of::<Q>())
            .expect("RingBuffer is not mapped properly");

        match unsafe { block.try_into_mut() } {
            Ok(value) => value,
            Err(WrongAlignment) => panic!("type Q should be trivially aligned"),
            Err(error) => panic!("unexpected error {:?}", error),
        }
    }


    /// Вычисляет размер заголовка одной записи для `RingBuffer` размера `REAL_SIZE`.
    const fn header_size() -> usize {
        let mut header_size = mem::size_of::<AtomicU8>();
        let mut size = REAL_SIZE;

        while size > 0 {
            header_size += mem::size_of::<u8>();
            size >>= u8::BITS;
        }

        header_size
    }
}


/// Читающая сторона [`RingBuffer`].
/// Позволяет создавать только читающие транзакции.
pub type ReadBuffer = RingBuffer<ReadTag>;


impl ReadBuffer {
    /// Создаёт читающую транзакцию.
    ///
    /// Возвращает [`None`], если [`RingBuffer`] был уже закрыт методом [`RingBuffer::close()`].
    pub fn read_tx(&mut self) -> Option<RingBufferReadTx<'_>> {
        let head = self.head;
        let tail = self.advance_tail()?;

        self.stats.txs += 1;

        Some(RingBufferTx {
            ring_buffer: self,
            header: head,
            head,
            tail,
            bytes: 0,
            _tag: PhantomData,
        })
    }


    /// Возвращает статистики читающих транзакций.
    pub fn read_stats(&self) -> &RingBufferStats {
        &self.stats
    }


    // ANCHOR: advance_tail
    /// Обновляет [`RingBuffer::tail`] и возвращает его.
    ///
    /// Если буфер закрыт, возвращает [`None`].
    /// Если замечает нарушение инвариантов буфера,
    /// считает что противоположная сторона испортила буфер и расценивает его как закрытый.
    fn advance_tail(&mut self) -> Option<usize> {
        // ANCHOR_END: advance_tail
        // TODO: your code here.
        None // TODO: remove before flight.
    }
}


/// Пишущая сторона [`RingBuffer`].
/// Позволяет создавать только пищущие транзакции.
pub type WriteBuffer = RingBuffer<WriteTag>;


impl WriteBuffer {
    /// Создаёт пишущую транзакцию.
    ///
    /// Возвращает [`None`], если [`RingBuffer`] был уже закрыт методом [`RingBuffer::close()`].
    pub fn write_tx(&mut self) -> Option<RingBufferWriteTx<'_>> {
        let head = self.advance_head()?;
        let tail = self.tail;

        self.stats.txs += 1;

        Some(RingBufferTx {
            ring_buffer: self,
            header: tail,
            head,
            tail,
            bytes: 0,
            _tag: PhantomData,
        })
    }


    /// Возвращает статистики пишущих транзакций.
    pub fn write_stats(&self) -> &RingBufferStats {
        &self.stats
    }


    // ANCHOR: advance_head
    /// Обновляет [`RingBuffer::head`] и возвращает его.
    ///
    /// Если буфер закрыт, возвращает [`None`].
    /// Если замечает нарушение инвариантов буфера,
    /// считает что противоположная сторона испортила буфер и расценивает его как закрытый.
    fn advance_head(&mut self) -> Option<usize> {
        // ANCHOR_END: advance_head
        // TODO: your code here.
        None // TODO: remove before flight.
    }
}


// ANCHOR: header
/// Заголовок одной записи [`RingBuffer`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum Header {
    /// Запись свободна, то есть в неё ещё ничего не записывалось.
    Clear = Self::CLEAR,

    /// Запись сообщает о закрытие буфера.
    Closed = Self::CLOSED,

    /// Запись закоммичена писателем, но ещё не закоммичена читателем.
    /// Так как она ещё нужна читателю, поверх неё пока нельзя писать новые данные,
    /// и она занимает место в буфере.
    Written {
        /// Размер полезных данных в записи.
        size: usize,
    } = Self::WRITTEN,

    /// Запись закоммичена и писателем и читателем.
    /// Теперь поверх неё можно писать новые данные.
    Read {
        /// Размер полезных данных в записи.
        size: usize,
    } = Self::READ,
}
// ANCHOR_END: header


impl Header {
    /// Значение флага для свободной записи.
    /// См. [`Header::Clear`].
    const CLEAR: u8 = 0;

    /// Значение флага для записи, закрывающей [`RingBuffer`].
    /// См. [`Header::Closed`].
    const CLOSED: u8 = 1;

    /// Значение флага для закоммиченной писателем и не закоммиченной читателем записи.
    /// См. [`Header::Written`].
    const WRITTEN: u8 = 2;

    /// Значение флага для закоммиченной и писателем и читателем записи.
    /// См. [`Header::Written`].
    const READ: u8 = 3;
}


// ANCHOR: ring_buffer_tx
/// Читающая или пишущая транзакция.
#[derive(Debug)]
pub struct RingBufferTx<'a, T: Tag> {
    /// Ссылка на исходный [`RingBuffer`].
    ring_buffer: &'a mut RingBuffer<T>,

    /// Позиция заголовка записи, которую обрабатывает эта транзакция.
    header: usize,

    /// Актуальное в рамках транзакции значение количества байт,
    /// прочитанных из буфера за всё время.
    /// В момент старта транзакции инициализируется из поля [`RingBuffer::head`].
    head: usize,

    /// Актуальное в рамках транзакции значение количества байт,
    /// записанных в буфер за всё время.
    /// В момент старта транзакции инициализируется из поля [`RingBuffer::tail`].
    tail: usize,

    /// Количество байт, прочитанных или записанных на текущий момент в данной транзакции.
    bytes: usize,

    /// Тег, отличающий пишущие транзакции от читающих.
    _tag: PhantomData<T>,
}
// ANCHOR_END: ring_buffer_tx


impl RingBufferTx<'_, ReadTag> {
    /// Возвращает в виде среза полезную нагрузку очередной записи из буфера.
    /// Или [`None`], если в этой читающей транзакции больше нет записей.
    /// Обновляет поля только самой транзакции [`RingBufferTx`].
    ///
    /// # Safety
    ///
    /// Так как возвращается слайс, расположенный в разделяемой памяти,
    /// его содержимое может конкурентно меняться другой стороной.
    /// То есть, нельзя проверить его на какие-либо инварианты,
    /// и после этого использовать, опираясь на них.
    /// Так как в промежутке эти инварианты могут быть сломаны другой стороной.
    ///
    /// То есть, нужно:
    ///   - Либо гарантировать, что другая сторона не запускается конкурентно.
    ///   - Либо прежде всего скопировать слайс, и дальше пользоваться только копией.
    pub unsafe fn read(&mut self) -> Option<&[u8]> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Коммитит читающую транзакцию, записывая обновлённое значение [`RingBuffer::head`] и
    /// статистику [`RingBuffer::stats`] в поля [`RingBuffer`].
    #[allow(unused_mut)] // TODO: remove before flight.
    pub fn commit(mut self) {
        // TODO: your code here.
        unimplemented!();
    }
}


impl RingBufferTx<'_, WriteTag> {
    /// Копирует в буфер байты среза `data`.
    /// Обновляет поля самой транзакции [`RingBufferTx`] и статистики [`RingBuffer::stats`],
    /// но не трогает поля [`RingBuffer::head`] и [`RingBuffer::tail`].
    /// Если в буфере не остаётся места под `data`, возвращает ошибку [`Error::Overflow`].
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        // TODO: your code here.
        unimplemented!();
    }


    /// Коммитит пишущую транзакцию, обновляя значение [`RingBuffer::tail`] и
    /// статистику [`RingBuffer::stats`] в полях [`RingBuffer`].
    #[allow(unused_mut)] // TODO: remove before flight.
    pub fn commit(mut self) {
        // TODO: your code here.
        unimplemented!();
    }


    /// Ёмкость, оставшаяся в буфере транзакции на текущий момент.
    pub fn capacity(&self) -> usize {
        let skip_header = if self.header == self.tail {
            HEADER_SIZE
        } else {
            0
        };
        if REAL_SIZE - (self.tail - self.head) >= skip_header + STATE_SIZE {
            REAL_SIZE - (self.tail + skip_header - self.head) - STATE_SIZE
        } else {
            0
        }
    }
}


impl<T: Tag> Drop for RingBufferTx<'_, T> {
    /// Обрывает читающую или пишущую транзакцию.
    /// Обновляет статистики [`RingBuffer::stats`],
    /// если в транзакции был прочитан или записан хотя бы один байт.
    fn drop(&mut self) {
        // TODO: your code here.
        unimplemented!();
    }
}


/// Тег, отличающий пишущие транзакции от читающих.
pub trait Tag {
    /// `true` если это писатель.
    const READ: bool;
}


/// Читающая транзакция.
pub type RingBufferReadTx<'a> = RingBufferTx<'a, ReadTag>;

/// Пишущая транзакция.
pub type RingBufferWriteTx<'a> = RingBufferTx<'a, WriteTag>;


/// Тег читающих транзакций.
#[derive(Debug, Default)]
pub struct ReadTag;


impl Tag for ReadTag {
    const READ: bool = true;
}


/// Тег пишущих транзакций.
#[derive(Debug, Default)]
pub struct WriteTag;


impl Tag for WriteTag {
    const READ: bool = false;
}


// ANCHOR: ring_buffer_stats
/// Статистики чтения или записи в буфер.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RingBufferStats {
    /// Количество байт, прочитанных или записанных в закоммиченных транзакциях.
    commited: usize,

    /// Количество закоммиченных транзакций соответствующего типа.
    commits: usize,

    /// Количество байт, прочитанных или записанных в оборванных транзакциях.
    dropped: usize,

    /// Количество оборванных (dropped, rolled back, aborted) транзакций соответствующего типа.
    drops: usize,

    /// Количество ошибок в транзакциях.
    errors: usize,

    /// Количество транзакций чтения либо записи соответственно.
    txs: usize,
}
// ANCHOR_END: ring_buffer_stats


// ANCHOR: error
/// Ошибки, которые могут возникать при работе с [`RingBuffer`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Буфер транзакции переполнен.
    Overflow {
        /// Место, остававшееся в буфере на момент старта транзакции.
        /// То есть, полная доступная для транзакции ёмкость.
        capacity: usize,

        /// Объём данных, уже записанных ранее в рамках транзакции.
        len: usize,

        /// Размер объекта, при записи которого буфер транзакции переполнился.
        /// Должно выполняться неравенство `exceeding_object_len > capacity - len`.
        exceeding_object_len: usize,
    },
}
// ANCHOR_END: error


/// Тип возвращаемого результата `T` или ошибки [`Error`] ---
/// мономорфизация [`result::Result`] по типу ошибки.
pub type Result<T> = result::Result<T, Error>;


/// Максимальный размер полезной нагрузки в одной записи [`RingBuffer`].
pub const MAX_CAPACITY: usize = REAL_SIZE - HEADER_SIZE - STATE_SIZE;


/// Размер заголовка одной записи в буфере.
const HEADER_SIZE: usize = ReadBuffer::header_size();

/// Размер хвоста после конца записи в буфере --- флага состояния следующей записи.
/// Используется при записи в буфер, чтобы отметить следующую запись свободной.
/// То есть, чтобы читающая сторона не пыталась читать следующую запись до тех пор,
/// пока та не будет записана на самом деле.
const STATE_SIZE: usize = mem::size_of::<AtomicU8>();

/// Размер отображённой виртуальной памяти.
const MAPPED_SIZE: usize = 2 * REAL_SIZE;

/// Размер занимаемой физической памяти.
const REAL_SIZE: usize = 4 * Page::SIZE;
