use core::{
    fmt,
    iter::Step,
    ops::{Add, Range, Sub},
};

use crate::error::{
    Error::{Overflow, WrongAlignment},
    Result,
};

use super::{
    addr::{Addr, PhysTag, Tag, VirtTag},
    size::{KiB, SizeOf},
};

// Used in docs.
#[allow(unused)]
use {
    super::{Phys, Virt},
    crate::error::Error,
};


/// Обобщённый тип для (виртуальных) страниц памяти и (физических) фреймов.
#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Frage<T: Tag>(Addr<T>);


impl<T: Tag> Frage<T> {
    /// Размер физического фрейма или виртуальной страницы.
    pub const SIZE: usize = 4 * KiB;


    /// Создаёт [`Frage`] --- [`Frame`] или [`Page`] ---
    /// по его начальному адресу `addr` --- [`Phys`] или [`Virt`] соответственно.
    ///
    /// Возвращает ошибку [`Error::WrongAlignment`] если `addr` не выровнен на [`Frage::SIZE`].
    pub fn new(addr: Addr<T>) -> Result<Self> {
        if addr.into_usize() % Self::SIZE == 0 {
            Ok(Self(addr))
        } else {
            Err(WrongAlignment)
        }
    }


    /// Возвращает нулевой [`Frage`] --- [`Frame`] или [`Page`].
    /// В отличие от [`Frage::default()`] доступна в константном контексте.
    pub const fn zero() -> Self {
        Self(Addr::zero())
    }


    /// Создаёт [`Frage`] --- [`Frame`] или [`Page`] ---
    /// по находящемуся внутри адресу `addr` --- [`Phys`] или [`Virt`] соответственно.
    pub fn containing(addr: Addr<T>) -> Self {
        Self(Addr::new(addr.into_usize() & !(Self::SIZE - 1)).unwrap())
    }


    /// Возвращает адрес [`Frage`] по его индексу `index`.
    ///
    /// Возвращает ошибку [`Error::Overflow`], если `index` превышает максимально допустимый.
    pub fn address_by_index(index: usize) -> Result<Addr<T>> {
        let address = index.checked_mul(Self::SIZE).ok_or(Overflow)?;
        Addr::new(address)
    }


    /// Создаёт [`Frage`] --- [`Frame`] или [`Page`] ---
    /// по его номеру `index`.
    ///
    /// Возвращает ошибку [`Error::Overflow`], если `index` превышает максимально допустимый.
    pub fn from_index(index: usize) -> Result<Self> {
        Ok(Self(Self::address_by_index(index)?))
    }


    /// Возвращает начальный адрес --- [`Phys`] или [`Virt`] ---
    /// для [`Frame`] или [`Page`] соответственно.
    pub fn address(&self) -> Addr<T> {
        self.0
    }


    /// Возвращает номер физического фрейма или виртуальной страницы.
    pub fn index(&self) -> usize {
        Self::index_by_address(self.address())
    }


    /// Возвращает номер физического фрейма или виртуальной страницы
    /// по адресу --- [`Phys`] или [`Virt`] соответственно.
    pub fn index_by_address(address: Addr<T>) -> usize {
        address.into_usize() / Self::SIZE
    }


    /// Возвращает смещение адреса внутри его физического фрейма или виртуальной страницы.
    pub fn offset(address: Addr<T>) -> usize {
        address.into_usize() % Self::SIZE
    }
}


impl<T: Tag> Add<usize> for Frage<T> {
    type Output = Result<Self>;


    fn add(self, rhs: usize) -> Self::Output {
        Self::new((self.0 + rhs * Self::SIZE)?)
    }
}


impl<T: Tag> Sub<usize> for Frage<T> {
    type Output = Result<Self>;


    fn sub(self, rhs: usize) -> Self::Output {
        Self::new((self.0 - rhs * Self::SIZE)?)
    }
}


impl<T: Tag> Sub<Self> for Frage<T> {
    type Output = Result<usize>;


    fn sub(self, rhs: Self) -> Self::Output {
        Ok((self.0 - rhs.0)? / Self::SIZE)
    }
}


impl<T: Tag> fmt::Debug for Frage<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{}({} @ {})",
            T::FRAGE_NAME,
            self.index(),
            self.address()
        )
    }
}


impl<T: Tag> fmt::Display for Frage<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{} @ {}", self.index(), self.address())
    }
}


impl<T: Tag> SizeOf for Frage<T> {
    const SIZE_OF: usize = Self::SIZE;
}


/// (Физический) фрейм памяти.
pub type Frame = Frage<PhysTag>;

/// (Виртуальная) страница памяти.
pub type Page = Frage<VirtTag>;


impl Page {
    /// Возвращает индекс виртуальной страницы,
    /// которая следует через `page_count` страниц после текущей.
    /// Если такой нет, то возвращает индекс на единицу больше
    /// чем у последней виртуальной страницы --- [`Page::higher_half_end_index()`].
    pub fn advance_index(&self, page_count: usize) -> usize {
        Self::forward_checked(*self, page_count)
            .map(|page| page.index())
            .unwrap_or_else(Page::higher_half_end_index)
    }


    /// Возвращает `true` если виртуальная страница относится к
    /// [верхней половине](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details).
    pub fn is_higher_half(&self) -> bool {
        Self::higher_half_start_index() <= self.index()
    }


    /// Возвращает `true` если виртуальная страница относится к
    /// [нижней половине](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details).
    pub fn is_lower_half(&self) -> bool {
        self.index() < Self::lower_half_end_index()
    }


    /// Возвращает первую виртуальную страницу
    /// [верхней половины](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details)
    /// адресного пространства.
    pub fn higher_half() -> Self {
        Page::containing(Virt::higher_half())
    }


    /// Возвращает первую виртуальную страницу
    /// [нижней половины](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details)
    /// адресного пространства.
    pub fn lower_half() -> Self {
        Page::containing(Virt::lower_half())
    }


    /// Возвращает индекс первой виртуальной страницы
    /// [верхней половины](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details)
    /// адресного пространства.
    pub fn higher_half_start_index() -> usize {
        Self::higher_half().index()
    }


    /// Возвращает индекс на единицу больше чем у последней виртуальной страницы
    /// [верхней половины](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details)
    /// адресного пространства.
    pub fn higher_half_end_index() -> usize {
        (usize::MAX / Self::SIZE) + 1
    }


    /// Возвращает индекс первой виртуальной страницы
    /// [нижней половины](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details)
    /// адресного пространства.
    pub fn lower_half_start_index() -> usize {
        Self::lower_half().index()
    }


    /// Возвращает индекс на единицу больше чем у последней виртуальной страницы
    /// [нижней половины](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details)
    /// адресного пространства.
    pub fn lower_half_end_index() -> usize {
        Self::lower_half_start_index() + Virt::half_size() / Self::SIZE
    }


    /// Возвращает запрещённый диапазон для номеров виртуальных страниц, находящийся между
    /// [двумя половинами](https://en.wikipedia.org/wiki/X86-64#Virtual_address_space_details)
    /// адресного пространства.
    fn non_canonical_range() -> Range<usize> {
        Self::lower_half_end_index()..Self::higher_half_start_index()
    }
}


impl Step for Page {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if start <= end {
            if Self::is_lower_half(start) == Self::is_lower_half(end) {
                Some(end.index() - start.index())
            } else {
                Some(end.index() - start.index() - Self::non_canonical_range().count())
            }
        } else {
            None
        }
    }


    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        let mut advanced = start.index().checked_add(count)?;

        if start.index() < Self::lower_half_end_index() && Self::lower_half_end_index() <= advanced
        {
            advanced = advanced.checked_add(Self::non_canonical_range().count())?;
        }

        Self::from_index(advanced).ok()
    }


    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        let mut advanced = start.index().checked_sub(count)?;

        if advanced < Self::higher_half_start_index() &&
            Self::higher_half_start_index() <= start.index()
        {
            advanced = advanced.checked_sub(Self::non_canonical_range().count())?;
        }

        Some(Self::from_index(advanced).expect("wrong advancement formula"))
    }
}


#[cfg(test)]
mod test {
    use static_assertions::const_assert;

    use super::{
        super::{
            addr::{Phys, PhysTag, Tag, VirtTag},
            mmu::PAGE_OFFSET_BITS,
        },
        Addr,
        Frage,
        Page,
    };


    const_assert!(Page::SIZE == 1 << PAGE_OFFSET_BITS);


    fn frage_alignment_check<T: Tag>() {
        for offest in 1..Frage::<T>::SIZE {
            assert!(Frage::<T>::new(Addr::<T>::new(offest).unwrap()).is_err());
        }
    }


    #[test]
    fn alignment_check() {
        frage_alignment_check::<PhysTag>();
        frage_alignment_check::<VirtTag>();
    }


    fn frage_address_and_index<T: Tag>(end_index: usize) {
        for index in (0..10).chain(end_index - 10..end_index) {
            let frage = Frage::<T>::from_index(index).unwrap();
            assert!(frage.index() == index);
            assert!(Frage::<T>::index_by_address(frage.address()) == index);
            assert!(Frage::<T>::new(frage.address()).unwrap() == frage);
        }

        let mut index = end_index;
        while index != 0 {
            assert!(Frage::<T>::from_index(index).is_err());
            index <<= 1;
        }
    }


    #[test]
    fn address_and_index() {
        const ADDRESS_SPACE_BITS: u32 = usize::BITS;

        frage_address_and_index::<PhysTag>(1 << (Phys::BITS - PAGE_OFFSET_BITS));
        frage_address_and_index::<VirtTag>(1 << (ADDRESS_SPACE_BITS - PAGE_OFFSET_BITS));
    }
}
