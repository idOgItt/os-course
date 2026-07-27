use core::{
    cmp,
    fmt,
    iter::IntoIterator,
    marker::PhantomData,
    mem,
    ops::Range,
    option::Option,
    ptr::NonNull,
};

use crate::error::{Error::InvalidArgument, Result};

use super::{
    addr::{Addr, GrouppedHex, IsVirt, Tag, Virt},
    frage::Frage,
    size,
    size::{Size, SizeOf},
};

// Used in docs.
#[allow(unused)]
use {
    super::{Frame, Page, Phys},
    crate::error::Error,
};


/// Абстракция куска физической или виртуальной памяти, постраничного или произвольного.
///
/// - [`Block<Phys>`] --- произвольный кусок физической памяти.
/// - [`Block<Virt>`] --- произвольный кусок виртуальной памяти.
/// - [`Block<Frame>`] --- набор последовательных физических фреймов.
/// - [`Block<Page>`] --- набор последовательных виртуальных страниц.
///
/// [`Block`] не владеет описываемой им памятью.
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct Block<T: Memory> {
    /// Номер первого элемента в блоке.
    start: usize,

    /// Номер следующего за последним элементом блока.
    end: usize,

    /// Фантомное, не занимающее памяти, поле.
    /// Служит для того чтобы сделать блоки с разными параметрами `T` несовместимыми.
    tag: PhantomData<T>,
}


impl<T: Memory> Block<T> {
    /// Создаёт блок для полуоткрытого интервала `[start, end)` базового типа `T`,
    /// который может быть [`Phys`], [`Virt`], [`Frame`] или [`Page`].
    pub fn new(start: T, end: T) -> Result<Self> {
        Self::from_index(T::index(start), T::index(end))
    }


    /// Возвращает пустой [`Block`].
    /// В отличие от [`Block::default()`] доступна в константном контексте.
    pub const fn zero() -> Self {
        Self {
            start: 0,
            end: 0,
            tag: PhantomData,
        }
    }


    /// Создаёт блок для полуоткрытого интервала `[start, end)` базового типа `T`,
    /// который задаётся своими индексами --- номерами байт для [`Phys`] и [`Virt`],
    /// номерами физических фреймов для [`Frame`] и номерами виртуальных страниц для [`Page`].
    pub fn from_index(start: usize, end: usize) -> Result<Self> {
        let start_address = T::address_by_index(start)?;
        let last_address = T::address_by_index(if start < end { end - 1 } else { end })?;

        if start <= end && T::is_same_area(start_address, last_address) {
            Ok(Self {
                start,
                end,
                tag: PhantomData,
            })
        } else {
            Err(InvalidArgument)
        }
    }


    /// Создаёт блок для полуоткрытого интервала `[start, end)` базового типа `T`,
    /// который задаётся своими индексами --- номерами байт для [`Phys`] и [`Virt`],
    /// номерами физических фреймов для [`Frame`] и номерами виртуальных страниц для [`Page`].
    ///
    /// Аналогичен [`Block::from_index()`], но индексы имеют тип [`u64`].
    pub fn from_index_u64(start: u64, end: u64) -> Result<Self> {
        Self::from_index(size::into_usize(start), size::into_usize(end))
    }


    /// Создаёт блок из одного элемента `element`.
    pub fn from_element(element: T) -> Result<Self> {
        Self::from_index(T::index(element), T::index(element) + 1)
    }


    /// Возвращает количество элементов в блоке, оно равно `Block::end - Block::start`.
    pub const fn count(&self) -> usize {
        self.end - self.start
    }


    /// Размер блока в байтах. Равно количеству элементов в блоке, умноженному на размер элемента.
    pub const fn size(&self) -> usize {
        self.count() * T::SIZE_OF
    }


    /// Индекс первого элемента в блоке.
    pub fn start(&self) -> usize {
        self.start
    }


    /// Индекс следующего за последним элементом в блоке.
    pub fn end(&self) -> usize {
        self.end
    }


    /// Возвращает адрес первого элемента в блоке.
    pub fn start_address(&self) -> T::Address {
        T::address_by_index(self.start).unwrap()
    }


    /// Возвращает адрес следующего за последним элементом блока.
    pub fn end_address(&self) -> Result<T::Address> {
        T::address_by_index(self.end)
    }


    /// Возвращает адрес следующего за последним элементом блока.
    pub fn end_address_u128(&self) -> u128 {
        u128::try_from(self.end()).unwrap() * u128::try_from(T::SIZE_OF).unwrap()
    }


    /// Проверяет, что заданный элемент относится к блоку.
    pub fn contains(&self, element: T) -> bool {
        self.contains_address(T::address(element))
    }


    /// Проверяет, что заданный адрес относится к блоку.
    pub fn contains_address(&self, addr: T::Address) -> bool {
        if addr < self.start_address() {
            false
        } else if let Ok(end_address) = self.end_address() {
            addr < end_address
        } else {
            true
        }
    }


    /// Проверяет, что заданный `block` целиком содержится внутри текущего или совпадает с ним.
    pub fn contains_block(&self, block: Block<T>) -> bool {
        self.start <= block.start && block.end <= self.end
    }


    /// Проверяет, что заданный `block` не пересекается с текущим.
    pub fn is_disjoint(&self, block: Block<T>) -> bool {
        self.is_empty() ||
            block.is_empty() ||
            (!self.contains_address(block.start_address()) &&
                !block.contains_address(self.start_address()))
    }


    /// Возвращает `true`, если блок пуст.
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }


    /// Возвращает пересечение блока `block` с текущим.
    pub fn intersection(&self, block: Block<T>) -> Block<T> {
        let start = cmp::max(self.start(), block.start());
        let end = cmp::min(self.end(), block.end());

        if start <= end {
            Block::from_index(start, end)
                .expect("an intersection of valid blocks should be a valid block")
        } else {
            Block::zero()
        }
    }


    /// Возвращает подблок исходного блока, задающийся диапазоном `range`.
    pub fn slice(&self, range: Range<usize>) -> Option<Self> {
        if range.start <= range.end && range.end <= self.count() {
            Self::from_index(self.start + range.start, self.start + range.end).ok()
        } else {
            None
        }
    }


    /// Разделяет блок на две дизъюнктные части: `self` и новый блок размером `count` единиц.
    ///
    /// Если в блоке недостаточно элементов, не меняя его возвращает `None`.
    pub fn tail(&mut self, count: usize) -> Option<Self> {
        // TODO: your code here.
        unimplemented!();
    }
}


impl<T: Tag> Block<Addr<T>> {
    /// Для заданного блока виртуальных или физических адресов
    /// возвращает минимальный содержащий его
    /// блок виртуальных страниц или физических фреймов соответственно.
    pub fn enclosing(&self) -> Block<Frage<T>> {
        let start_frage = Frage::<T>::index_by_address(self.start_address());
        let end_frage = if self.start == self.end {
            start_frage
        } else {
            Frage::<T>::index_by_address((self.end_address().unwrap() - 1).unwrap()) + 1
        };

        Block::from_index(start_frage, end_frage).unwrap()
    }
}


impl<T: Memory> Block<T>
where
    T::Address: IsVirt,
{
    /// Преобразует [`Block<Virt>`] или [`Block<Page>`] в указатель на константный `Q`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::InvalidArgument`] если блок не соответствует типу `Q` по размеру.
    ///   - [`Error::WrongAlignment`] если блок не соответствует типу `Q` по выравниванию.
    pub fn try_into_ptr<Q>(self) -> Result<*const Q> {
        if self.size() == mem::size_of::<Q>() {
            self.start_address().try_into_ptr::<Q>()
        } else {
            Err(InvalidArgument)
        }
    }


    /// Преобразует [`Block<Virt>`] или [`Block<Page>`] в указатель на мутабельный `Q`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::InvalidArgument`] если блок не соответствует типу `Q` по размеру.
    ///   - [`Error::WrongAlignment`] если блок не соответствует типу `Q` по выравниванию.
    pub fn try_into_mut_ptr<Q>(self) -> Result<*mut Q> {
        if self.size() == mem::size_of::<Q>() {
            self.start_address().try_into_mut_ptr::<Q>()
        } else {
            Err(InvalidArgument)
        }
    }


    /// Преобразует [`Block<Virt>`] или [`Block<Page>`] в иммутабельную ссылку на `Q`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::InvalidArgument`] если блок не соответствует типу `Q` по размеру.
    ///   - [`Error::WrongAlignment`] если блок не соответствует типу `Q` по выравниванию.
    ///   - [`Error::Null`] если адрес блока нулевой.
    ///
    /// # Safety
    ///
    /// Вызывающая сторона должна гарантировать,
    /// что не будут нарушены инварианты управления памятью в Rust,
    /// которые не относятся к возвращаемым ошибкам.
    pub unsafe fn try_into_ref<'a, Q>(self) -> Result<&'a Q> {
        if self.size() == mem::size_of::<Q>() {
            let start_address = self.start_address();
            unsafe { start_address.try_into_ref::<Q>() }
        } else {
            Err(InvalidArgument)
        }
    }


    /// Преобразует [`Block<Virt>`] или [`Block<Page>`] в мутабельную ссылку на `Q`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::InvalidArgument`] если блок не соответствует типу `Q` по размеру.
    ///   - [`Error::WrongAlignment`] если блок не соответствует типу `Q` по выравниванию.
    ///   - [`Error::Null`] если адрес блока нулевой.
    ///
    /// # Safety
    ///
    /// Вызывающая сторона должна гарантировать,
    /// что не будут нарушены инварианты управления памятью в Rust,
    /// которые не относятся к возвращаемым ошибкам.
    pub unsafe fn try_into_mut<'a, Q>(self) -> Result<&'a mut Q> {
        if self.size() == mem::size_of::<Q>() {
            let start_address = self.start_address();
            unsafe { start_address.try_into_mut::<Q>() }
        } else {
            Err(InvalidArgument)
        }
    }


    /// Преобразует [`Block<Virt>`] или [`Block<Page>`] в срез иммутабельных элементов типа `Q`.
    /// Размер среза вычисляется из размера блока и размера типа `Q`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::InvalidArgument`] если блок не соответствует типу `Q` по размеру.
    ///   - [`Error::WrongAlignment`] если блок не соответствует типу `Q` по выравниванию.
    ///   - [`Error::Null`] если адрес блока нулевой.
    ///
    /// # Safety
    ///
    /// Вызывающая сторона должна гарантировать,
    /// что не будут нарушены инварианты управления памятью в Rust,
    /// которые не относятся к возвращаемым ошибкам.
    pub unsafe fn try_into_slice<'a, Q>(self) -> Result<&'a [Q]> {
        if self.size() % mem::size_of::<Q>() == 0 {
            let start_address = self.start_address();
            let len = self.size() / mem::size_of::<Q>();
            unsafe { start_address.try_into_slice::<Q>(len) }
        } else {
            Err(InvalidArgument)
        }
    }


    /// Преобразует [`Block<Virt>`] или [`Block<Page>`] в срез мутабельных элементов типа `Q`.
    /// Размер среза вычисляется из размера блока и размера типа `Q`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::InvalidArgument`] если блок не соответствует типу `Q` по размеру.
    ///   - [`Error::WrongAlignment`] если блок не соответствует типу `Q` по выравниванию.
    ///   - [`Error::Null`] если адрес блока нулевой.
    ///
    /// # Safety
    ///
    /// Вызывающая сторона должна гарантировать,
    /// что не будут нарушены инварианты управления памятью в Rust,
    /// которые не относятся к возвращаемым ошибкам.
    pub unsafe fn try_into_mut_slice<'a, Q>(self) -> Result<&'a mut [Q]> {
        if self.size() % mem::size_of::<Q>() == 0 {
            let start_address = self.start_address();
            let len = self.size() / mem::size_of::<Q>();
            unsafe { start_address.try_into_mut_slice::<Q>(len) }
        } else {
            Err(InvalidArgument)
        }
    }


    /// Преобразует [`Block<Virt>`] или [`Block<Page>`] в [`NonNull<[Q]>`].
    /// Размер среза вычисляется из размера блока и размера типа `Q`.
    ///
    /// Возвращает ошибки:
    ///   - [`Error::InvalidArgument`] если блок не соответствует типу `Q` по размеру.
    ///   - [`Error::WrongAlignment`] если блок не соответствует типу `Q` по выравниванию.
    ///   - [`Error::Null`] если адрес блока нулевой.
    ///
    /// # Safety
    ///
    /// Вызывающая сторона должна гарантировать,
    /// что не будут нарушены инварианты управления памятью в Rust,
    /// которые не относятся к возвращаемым ошибкам.
    pub unsafe fn try_into_non_null_slice<Q>(self) -> Result<NonNull<[Q]>> {
        let slice = unsafe { self.try_into_mut_slice()? };
        Ok(slice.into())
    }
}


impl Block<Virt> {
    /// Преобразует указатель на `T` в [`Block<Virt>`].
    ///
    /// # Panics
    ///
    /// Паникует, если указатель не корректен.
    pub fn from_ptr<T>(x: *const T) -> Self {
        let start = Virt::from_ptr(x);
        let end = (start + mem::size_of::<T>()).unwrap();
        Self::new(start, end).unwrap()
    }


    /// Преобразует указатель на `T` в [`Block<Virt>`].
    ///
    /// # Panics
    ///
    /// Паникует, если указатель не корректен.
    pub fn from_mut_ptr<T>(x: *mut T) -> Self {
        Self::from_ptr(x)
    }


    /// Преобразует ссылку на `T` в [`Block<Virt>`].
    ///
    /// # Panics
    ///
    /// Паникует, если ссылка не корректна.
    pub fn from_ref<T>(x: &T) -> Self {
        Self::from_ptr(x)
    }


    /// Преобразует ссылку на `T` в [`Block<Virt>`].
    ///
    /// # Panics
    ///
    /// Паникует, если ссылка не корректна.
    pub fn from_mut<T>(x: &mut T) -> Self {
        Self::from_mut_ptr(x)
    }


    /// Преобразует срез элементов типа `T` в [`Block<Virt>`].
    ///
    /// # Panics
    ///
    /// Паникует, если срез не корректен.
    pub fn from_slice<T>(x: &[T]) -> Self {
        let range = x.as_ptr_range();

        Self::new(Virt::from_ptr(range.start), Virt::from_ptr(range.end)).unwrap()
    }
}


impl<T: Tag> From<Block<Frage<T>>> for Block<Addr<T>> {
    /// Преобразует блок виртуальных страниц или физических фреймов
    /// в блок виртуальных или физических адресов соответственно.
    fn from(block: Block<Frage<T>>) -> Self {
        Self::new(block.start_address(), block.end_address().unwrap()).unwrap()
    }
}


impl<T: Memory> fmt::Debug for Block<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{}, {} count {}, [~{}, ",
            self,
            T::NAME,
            self.count(),
            Size::new::<T>(self.start),
        )?;

        if let Ok(end_address) = usize::try_from(self.end_address_u128()) {
            write!(formatter, "~{})", Size::bytes(end_address))
        } else {
            write!(formatter, "~16.000EiB)")
        }
    }
}


impl<T: Memory> fmt::Display for Block<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "[{}, {}{}), size {}",
            self.start_address(),
            T::HEX_PREFIX,
            GrouppedHex::new(self.end_address_u128()),
            Size::new::<T>(self.count()),
        )
    }
}


impl<T: Memory> IntoIterator for Block<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;


    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.start..self.end, PhantomData)
    }
}


/// Итератор по блоку.
///
/// В качестве итератора нельзя использовать сам [`Block`], так как он реализует типаж [`Copy`].
/// А делать итератор копируемым чревато ошибками.
#[derive(Clone)]
pub struct IntoIter<T: Memory>(Range<usize>, PhantomData<T>);


impl<T: Memory> Iterator for IntoIter<T> {
    type Item = T;


    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().and_then(|index| T::from_index(index).ok())
    }
}


/// Описывает базовые типы для [`Block`] и их операции.
pub trait Memory: Clone + Copy + SizeOf {
    /// Заменяет префикс `0x` при печати на:
    ///   - `0p` (**p**hysical) для физических адресов;
    ///   - `0v` (**v**irtual) для виртуальных адресов.
    const HEX_PREFIX: &'static str;

    /// Имя базового типа для отладочной печати блока.
    const NAME: &'static str;


    /// Тип адреса для базового типа:
    ///   - [`Virt`] для [`Virt`] и [`Page`];
    ///   - [`Phys`] для [`Phys`] и [`Frame`].
    type Address: fmt::Display + Ord;


    /// Возвращает адрес базового элемента `element`:
    ///   - `element` для `Addr`;
    ///   - `element.address()` для `Frage`.
    fn address(element: Self) -> Self::Address;

    /// Возвращает адрес базового элемента с индексом `index`:
    ///   - `Addr::new(index)` для `Addr`;
    ///   - `Frage::address_by_index(index)` для `Frage`.
    fn address_by_index(index: usize) -> Result<Self::Address>;

    /// Возвращает базовый элемент по его индексу `index`:
    ///   - `Addr::new(index)` для `Addr`;
    ///   - `Frage::from_index(index)` для `Frage`.
    fn from_index(index: usize) -> Result<Self>;

    /// Возвращает индекс базового элемента `element`:
    ///   - `element.into_usize()` для `Addr`;
    ///   - `element.index()` для `Frage`.
    fn index(element: Self) -> usize;

    /// Возвращает `true` если:
    ///   - Адреса `x` и `y` виртуальные и находятся в одной и той же
    ///     [половине виртуального адресного пространства](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details).
    ///   - Адреса `x` и `y` физические.
    fn is_same_area(x: Self::Address, y: Self::Address) -> bool;
}


impl<T: Tag> Memory for Addr<T> {
    const HEX_PREFIX: &'static str = T::HEX_PREFIX;
    const NAME: &'static str = T::ADDR_NAME;


    type Address = Addr<T>;


    fn address(element: Self) -> Self::Address {
        element
    }


    fn address_by_index(index: usize) -> Result<Self::Address> {
        Addr::<T>::new(index)
    }


    fn from_index(index: usize) -> Result<Self> {
        Self::new(index)
    }


    fn index(element: Self) -> usize {
        element.into_usize()
    }


    fn is_same_area(x: Self::Address, y: Self::Address) -> bool {
        T::is_same_area(x, y)
    }
}


impl<T: Tag> Memory for Frage<T> {
    const HEX_PREFIX: &'static str = T::HEX_PREFIX;
    const NAME: &'static str = T::FRAGE_NAME;


    type Address = Addr<T>;


    fn address(element: Self) -> Self::Address {
        element.address()
    }


    fn address_by_index(index: usize) -> Result<Self::Address> {
        Frage::<T>::address_by_index(index)
    }


    fn from_index(index: usize) -> Result<Self> {
        Self::from_index(index)
    }


    fn index(element: Self) -> usize {
        element.index()
    }


    fn is_same_area(x: Self::Address, y: Self::Address) -> bool {
        T::is_same_area(x, y)
    }
}


#[cfg(test)]
mod test {
    use crate::error::Error::{InvalidArgument, Overflow};

    use super::{
        super::{Page, Phys, Virt},
        Block,
    };


    #[test]
    fn grow_forward() {
        assert!(Block::<Phys>::from_index(0, 1).is_ok());
        assert!(Block::<Phys>::from_index(1, 1).is_ok());
        assert_eq!(Block::<Phys>::from_index(1, 0), Err(InvalidArgument));
    }


    #[test]
    fn full_halves() {
        let lower_half_first_page = LOWER_HALF_FIRST / Page::SIZE;
        let lower_half_last_page = LOWER_HALF_LAST / Page::SIZE;
        let higher_half_first_page = HIGHER_HALF_FIRST / Page::SIZE;
        let higher_half_last_page = HIGHER_HALF_LAST / Page::SIZE;

        let half_size = LOWER_HALF_LAST - LOWER_HALF_FIRST + 1;
        assert_eq!(half_size, HIGHER_HALF_LAST - HIGHER_HALF_FIRST + 1);

        // This way it works.
        let full_lower_half =
            Block::<Page>::from_index(lower_half_first_page, lower_half_last_page + 1).unwrap();
        assert!(full_lower_half.contains_address(Virt::new(LOWER_HALF_FIRST).unwrap()));
        assert!(full_lower_half.contains_address(Virt::new(LOWER_HALF_LAST).unwrap()));
        assert_eq!(full_lower_half.size(), half_size);
        assert_eq!(full_lower_half.end_address(), Err(InvalidArgument));
        assert_eq!(
            Block::<Page>::from_index(lower_half_first_page, lower_half_last_page + 2),
            Err(InvalidArgument),
        );
        assert_eq!(
            Block::<Page>::from_index(
                lower_half_first_page.wrapping_sub(1),
                lower_half_last_page + 1,
            ),
            Err(Overflow),
        );

        // But this way it should not:
        // ```rust
        // Block::new(
        //     Page::from_index(lower_half_first_page).unwrap(),
        //     Page::from_index(lower_half_last_page + 1).unwrap(),
        // )
        // ```
        // Because `Page::from_index(lower_half_last_page + 1)` has an invalid virtual address:
        assert_eq!(
            Page::from_index(lower_half_last_page + 1),
            Err(InvalidArgument),
        );

        let full_higher_half =
            Block::<Page>::from_index(higher_half_first_page, higher_half_last_page + 1).unwrap();
        assert!(full_higher_half.contains_address(Virt::new(HIGHER_HALF_LAST).unwrap()));
        assert_eq!(full_higher_half.end_address(), Err(Overflow));
        assert_eq!(full_higher_half.size(), half_size);
        assert_eq!(
            Block::<Page>::from_index(higher_half_first_page, higher_half_last_page + 2),
            Err(Overflow),
        );
        assert_eq!(
            Block::<Page>::from_index(higher_half_first_page - 1, higher_half_last_page + 1),
            Err(InvalidArgument),
        );

        let full_lower_half =
            Block::<Virt>::from_index(LOWER_HALF_FIRST, LOWER_HALF_LAST + 1).unwrap();
        assert!(full_lower_half.contains_address(Virt::new(LOWER_HALF_FIRST).unwrap()));
        assert!(full_lower_half.contains_address(Virt::new(LOWER_HALF_LAST).unwrap()));
        assert_eq!(full_lower_half.size(), half_size);
        assert_eq!(full_lower_half.end_address(), Err(InvalidArgument));
        assert_eq!(
            Block::<Virt>::from_index(LOWER_HALF_FIRST, LOWER_HALF_LAST + 2),
            Err(InvalidArgument),
        );
        assert_eq!(
            Block::<Virt>::from_index(LOWER_HALF_FIRST.wrapping_sub(1), LOWER_HALF_LAST + 1),
            Err(InvalidArgument),
        );

        // `HIGHER_HALF_LAST + 1` will overflow so
        // full higher half `Block<Virt>` is impossible to create and store.
        let full_higher_half =
            Block::<Virt>::from_index(HIGHER_HALF_FIRST, HIGHER_HALF_LAST.wrapping_add(1));
        assert_eq!(full_higher_half, Err(InvalidArgument));
        assert_eq!(
            Block::<Virt>::from_index(HIGHER_HALF_FIRST, HIGHER_HALF_LAST.wrapping_add(2)),
            Err(InvalidArgument),
        );
        assert_eq!(
            Block::<Virt>::from_index(HIGHER_HALF_FIRST - 1, HIGHER_HALF_LAST.wrapping_add(1)),
            Err(InvalidArgument),
        );
    }


    #[test]
    fn enforce_same_virt_area() {
        assert!(Block::<Virt>::from_index(0, LOWER_HALF_LAST).is_ok());
        assert!(Block::<Virt>::from_index(0, LOWER_HALF_LAST + 1).is_ok());

        let inside_lower_half = Virt::new(INSIDE_LOWER_HALF).unwrap();
        let inside_higher_half = Virt::new(INSIDE_HIGHER_HALF).unwrap();
        assert_eq!(
            Block::new(inside_lower_half, inside_higher_half),
            Err(InvalidArgument)
        );
        assert_eq!(
            Block::<Virt>::from_index(INSIDE_LOWER_HALF, INSIDE_HIGHER_HALF),
            Err(InvalidArgument)
        );

        let lower_half_last =
            Page::new(Virt::new(LOWER_HALF_LAST - (Page::SIZE - 1)).unwrap()).unwrap();
        let inside_lower_half = Page::new(inside_lower_half).unwrap();
        let inside_higher_half = Page::new(inside_higher_half).unwrap();
        assert!(Block::<Page>::from_index(0, lower_half_last.index()).is_ok());
        assert!(Block::<Page>::from_index(0, lower_half_last.index() + 1).is_ok());
        assert_eq!(
            Block::<Page>::from_index(0, lower_half_last.index() + 2),
            Err(InvalidArgument)
        );
        assert_eq!(
            Block::new(inside_lower_half, inside_higher_half),
            Err(InvalidArgument)
        );
        assert_eq!(
            Block::<Page>::from_index(inside_lower_half.index(), inside_higher_half.index()),
            Err(InvalidArgument)
        );
    }


    #[test]
    fn enclosing() {
        for base in [
            LOWER_HALF_FIRST,
            HIGHER_HALF_FIRST,
            INSIDE_LOWER_HALF,
            INSIDE_HIGHER_HALF,
        ] {
            for shift in [0, 1] {
                let start_virt = base + shift;
                let start_page = start_virt / Page::SIZE;
                for (end_virt, end_page) in [
                    (start_virt, start_page),
                    (start_virt + 1, start_page + 1),
                    (start_virt + (Page::SIZE - shift) - 1, start_page + 1),
                    (start_virt + (Page::SIZE - shift), start_page + 1),
                    (start_virt + (Page::SIZE - shift) + 1, start_page + 2),
                ] {
                    assert!(
                        Block::<Virt>::from_index(start_virt, end_virt).unwrap().enclosing() ==
                            Block::<Page>::from_index(start_page, end_page).unwrap()
                    );
                }
            }
        }

        for base in [
            LOWER_HALF_FIRST + 2 * Page::SIZE - 1,
            LOWER_HALF_LAST,
            HIGHER_HALF_FIRST + 2 * Page::SIZE - 1,
            HIGHER_HALF_LAST,
        ] {
            for shift in [0, 1] {
                let end_virt = base - shift;
                let end_page = end_virt / Page::SIZE + 1;
                for (start_virt, start_page) in [
                    (end_virt - 1, end_page - 1),
                    (end_virt - (Page::SIZE - shift) + 1, end_page - 1),
                    (end_virt - (Page::SIZE - shift), end_page - 2),
                    (end_virt - (Page::SIZE - shift) - 1, end_page - 2),
                ] {
                    assert_eq!(
                        Block::<Virt>::from_index(start_virt, end_virt).unwrap().enclosing(),
                        Block::<Page>::from_index(start_page, end_page).unwrap(),
                    );
                }
                assert_eq!(
                    Block::<Virt>::from_index(end_virt, end_virt).unwrap().enclosing(),
                    Block::<Page>::from_index(end_page - 1, end_page - 1).unwrap(),
                );
            }
        }
    }


    #[test]
    fn bad_address() {
        let phys_end = 1 << 52;
        assert!(Block::<Phys>::from_index(phys_end - 1, phys_end).is_ok());
        assert_eq!(
            Block::<Phys>::from_index(phys_end, phys_end),
            Err(InvalidArgument),
        );
        assert_eq!(
            Block::<Phys>::from_index(phys_end - 1, phys_end + 1),
            Err(InvalidArgument),
        );

        let bad_virt = 1 << 48;
        assert_eq!(
            Block::<Virt>::from_index(bad_virt, bad_virt),
            Err(InvalidArgument),
        );
    }


    const LOWER_HALF_FIRST: usize = 0x0;
    const LOWER_HALF_LAST: usize = 0x0000_7FFF_FFFF_FFFF;
    const HIGHER_HALF_FIRST: usize = 0xFFFF_8000_0000_0000;
    const HIGHER_HALF_LAST: usize = 0xFFFF_FFFF_FFFF_FFFF;
    const INSIDE_LOWER_HALF: usize = (LOWER_HALF_LAST / (2 * Page::SIZE)) * Page::SIZE;
    const INSIDE_HIGHER_HALF: usize = 0xFFFF_FFFF_0000_0000;
}
