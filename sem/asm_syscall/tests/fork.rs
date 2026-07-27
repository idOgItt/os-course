use asm_syscall as syscall;


#[test]
fn test_fork() {
    let p =  { syscall::fork() };
    if p == 0 {
         { syscall::exit(0); }
    }
    let (r, status) =  { syscall::waitpid(p, 0) };
    assert_eq!(r, p);
    assert_eq!(status & 0x7F, 0);
    assert_eq!((status >> 8) & 0xFF, 0);
}
