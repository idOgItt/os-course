use std::{ffi::CString, vec};

use asm_syscall as syscall;


#[test]
fn test_exec() {
    let mut pipefd = [-1; 2];
    syscall::pipe(&mut pipefd);

    let p = syscall::fork();
    if p == 0 {
        let argv: Vec<_> =
            vec!["/bin/echo", "hi"].iter().map(|&x| CString::new(x).unwrap()).collect();
        let envp = vec![];
        syscall::close(pipefd[0]);
        syscall::close(1);
        syscall::dup(pipefd[1]);
        syscall::execve(&argv[0], &argv, &envp);
        syscall::exit(1);
    }

    syscall::close(pipefd[1]);

    let mut buf = [0; 10];
    let r = syscall::read(pipefd[0], &mut buf);
    assert_eq!(r, 3);
    assert_eq!(&buf[0..3], "hi\n".as_bytes());

    let (s, status) = syscall::waitpid(p, 0);
    assert_eq!(s, p);
    assert_eq!(status & 0x7F, 0);
}
