/// Определения типов адресов памяти, как виртуальных, так и физических ---
/// [`Addr`], [`Virt`] и [`Phys`].
pub mod addr;

/// Работа с блоками памяти [`Block`].
pub mod block;

/// Определения типов (виртуальных) страниц памяти и (физических) фреймов ---
/// [`Frage`], [`Frame`] и [`Page`].
pub mod frage;

/// Работа с блоком управления памятью процессора ---
/// [Memory Management Unit](https://en.wikipedia.org/wiki/Memory_management_unit).
pub mod mmu;

/// Информация об
/// [исключении доступа к странице](https://en.wikipedia.org/wiki/Page_fault)
/// (Page Fault) --- [`PageFaultInfo`].
pub mod page_fault_info;

/// Абстракция размера в памяти [`Size`].
pub mod size;


pub use addr::{Phys, Virt};
pub use block::Block;
pub use frage::{Frame, Page};
pub use mmu::{KERNEL_RW, USER, USER_RW};
pub use page_fault_info::PageFaultInfo;
pub use size::{Size, SizeOf};

// Used in docs.
#[allow(unused)]
use self::{
    addr::Addr,
    frage::Frage,
    mmu::{PageTable, PageTableEntry, PageTableFlags},
};
