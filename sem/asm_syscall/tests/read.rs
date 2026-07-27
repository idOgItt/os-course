use std::ffi::CString;

use asm_syscall as syscall;


#[test]
fn test_read() {
    let mut buf = [0; 10];
    let f =  { syscall::open(&CString::new("test_read.source").unwrap(), 0) };
    let r =  { syscall::read(f, &mut buf) };
    assert_eq!(r, 5);
    assert_eq!(&buf[0..5], "word\n".as_bytes());
}
