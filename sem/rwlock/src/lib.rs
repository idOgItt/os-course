#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#![deny(warnings)]
#![no_std]

mod sync;

pub use sync::{ReadGuard, RwLock, WriteGuard};
