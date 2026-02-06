; Simple userspace test program for lymadOS
; Assembles to a flat binary that loops making syscalls
;
; Build with:
;   nasm -f bin -o hello.bin hello.asm
;
; Or as ELF:
;   nasm -f elf64 -o hello.o hello.asm
;   ld -nostdlib -static -Ttext=0x400000 -o hello.elf hello.o
;   objcopy -O binary hello.elf hello.bin

BITS 64

; ELF header will place us at 0x400000
section .text
global _start

_start:
    ; Counter for syscall argument
    mov r12, 0

.loop:
    ; Syscall: print counter
    ; rax = syscall number (1 = write/print)
    ; rdi = arg1 (fd = 1 for stdout)  
    ; rsi = arg2 (our counter value)
    ; rdx = arg3 (unused)
    mov rax, 1          ; syscall number 1 = "print"
    mov rdi, 1          ; fd = 1 (stdout)
    mov rsi, r12        ; counter value
    mov rdx, 0          ; unused
    syscall

    ; Increment counter
    inc r12

    ; Small delay loop
    mov rcx, 0x100000
.delay:
    pause
    dec rcx
    jnz .delay

    ; Loop forever
    jmp .loop
