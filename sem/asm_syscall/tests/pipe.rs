use asm_syscall as syscall;


#[test]
fn test_pipe() {
    let mut pipefd = [-1; 2];
    let mut buf = [0; 10];
    syscall::pipe(&mut pipefd);
    syscall::write(pipefd[1], "test".as_bytes());
    let r = syscall::read(pipefd[0], &mut buf);
    assert_eq!(r, 4);
    assert_eq!(&buf[0..4], "test".as_bytes());
}
