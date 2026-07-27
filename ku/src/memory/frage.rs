use core::fmt;

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
