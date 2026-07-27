.intel_syntax noprefix

.text

.global atomic_load
.global atomic_store
.global atomic_add
.global atomic_sub
.global atomic_xchg
.global atomic_cmpxchg

atomic_load:
    mov rax, [rdi]
    ret

atomic_store:
    mov [rdi], rsi
    mfence
    ret

atomic_add:
    lock add [rdi], rsi
    ret

atomic_sub:
    lock sub [rdi], rsi
    ret

atomic_xchg:
    xchg rax, [rdi]
    ret

atomic_cmpxchg:
    mov rax, [rsi]
    lock cmpxchg [rdi], rdx
    mov [rsi], rax
    sete al
    movzx eax, al
    ret
