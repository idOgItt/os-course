use core::cmp;

use xmas_elf::{
    program::{ProgramHeader, Type},
    ElfFile,
};

use crate::{
    allocator::BigAllocator,
    error::{Error::Elf, Result},
    log::debug,
    memory::{size, Block, Virt, USER_RW},
};


/// Загружает [ELF--файл](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format)
/// `file`.
/// Выделяет для него память с помощью `allocator`.
///
/// Вызывающая её функция должна гарантировать,
/// что `allocator` выделяет память в текущем адресном пространстве.
pub fn load<T: BigAllocator>(allocator: &mut T, file: &[u8]) -> Result<Virt> {
    // TODO: your code here.
    unimplemented!();
}


/// Загружает сегмент `program_header`
/// [ELF--файла](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format).
/// Выделяет для него память с помощью `allocator`.
/// В аргументе `mapped_end` поддерживает адрес до которого (не включительно)
/// память для загружаемого процесса уже аллоцирована.
///
/// Вызывающая её функция должна гарантировать,
/// что `allocator` выделяет память в текущем адресном пространстве.
fn load_program_header<T: BigAllocator>(
    allocator: &mut T,
    program_header: &ProgramHeader,
    file: &[u8],
    mapped_end: &mut Virt,
) -> Result<()> {
    // TODO: your code here.
    unimplemented!();
}


/// Расширяет отображение текущего адресного пространства,
/// чтобы гарантировать что `memory_block` отображён в память.
/// Выделяет для него память с помощью `allocator`.
/// В аргументе `mapped_end` поддерживает адрес до которого (не включительно)
/// память для загружаемого процесса уже аллоцирована.
/// Новые страницы отображения заполняет нулями.
///
/// Вызывающая её функция должна гарантировать,
/// что `allocator` выделяет память в текущем адресном пространстве.
fn extend_mapping<T: BigAllocator>(
    allocator: &mut T,
    memory_block: &Block<Virt>,
    mapped_end: &mut Virt,
) -> Result<()> {
    // TODO: your code here.
    unimplemented!();
}


/// Для сегмента `program_header`
/// [ELF--файла](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format)
/// возвращает соответствующий ему описатель блока памяти.
fn memory_block(program_header: &ProgramHeader) -> Result<Block<Virt>> {
    // TODO: your code here.
    unimplemented!();
}


#[cfg(test)]
mod test {
    use core::alloc::Layout;

    use xmas_elf::program::{ProgramHeader, ProgramHeader64};

    use crate::{
        allocator::BigAllocator,
        error::Result,
        memory::{size, Block, Page, Virt},
    };

    use super::load_program_header;


    #[ignore] // TODO: remove before flight.
    #[test]
    fn load_program_header_from_correct_offset() {
        const DATA: [u8; 3] = [1, 2, 3];
        const OFFSET: usize = 7;

        let mut file = [0; OFFSET + DATA.len()];
        file[OFFSET..OFFSET + DATA.len()].copy_from_slice(&DATA);

        let mut target = [0; DATA.len()];

        let mut ph64 = ProgramHeader64::default();
        ph64.virtual_addr = Virt::from_mut_ptr(target.as_mut_ptr()).into_u64();
        ph64.offset = size::into_u64(OFFSET);
        ph64.file_size = size::into_u64(DATA.len());
        ph64.mem_size = size::into_u64(DATA.len());

        let ph = ProgramHeader::Ph64(&ph64);

        let mut mapped_end = Virt::new(usize::MAX).unwrap();

        let result = load_program_header(&mut DummyAllocator, &ph, &file, &mut mapped_end);

        assert!(result.is_ok());
        assert_eq!(&file[OFFSET..OFFSET + DATA.len()], &target[..DATA.len()]);
    }


    struct DummyAllocator;


    unsafe impl BigAllocator for DummyAllocator {
        fn flags(&self) -> PageTableFlags {
            unimplemented!();
        }


        fn set_flags(&mut self, _flags: PageTableFlags) -> Result<()> {
            unimplemented!();
        }


        fn reserve(&mut self, _layout: Layout) -> Result<Block<Page>> {
            unimplemented!();
        }


        unsafe fn unreserve(&mut self, _block: Block<Page>) -> Result<()> {
            unimplemented!();
        }


        unsafe fn rereserve(
            &mut self,
            _old_block: Block<Page>,
            _sub_block: Block<Page>,
        ) -> Result<()> {
            unimplemented!();
        }


        unsafe fn map(&mut self, _block: Block<Page>, _flags: PageTableFlags) -> Result<()> {
            unimplemented!();
        }


        unsafe fn unmap(&mut self, _block: Block<Page>) -> Result<()> {
            unimplemented!();
        }


        unsafe fn copy_mapping(
            &mut self,
            _old_block: Block<Page>,
            _new_block: Block<Page>,
            _flags: Option<PageTableFlags>,
        ) -> Result<()> {
            unimplemented!();
        }
    }
}
