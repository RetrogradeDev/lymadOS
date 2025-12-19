This is my seconds OS project. It will be written in Rust and aim to be a simple Unix-like OS, currently targeting x86_64 architecture.

I'll try to document my progress in JOURNAL.md (or not, idk how boot will work), but don't expect too much structure.

It should should have:
- [x] working memory management 
- [ ] kernel mapped in the lower half
- [ ] FAT32 filesystem
- [ ] interrupts
- [x] UEFI bootloader
- [ ] APIC timer
- [ ] syscalls
- [ ] task scheduler
- [ ] userland
- [ ] elf parser
- [ ] VFS
- [ ] basic compatibility with Linux syscalls
- [ ] desktop??
- [ ] mouse/keyboard drivers
- [ ] if possible basic networking
- [ ] test runner

Cool achievemnts:
- run doom
- run GCC
- ping google.com

Rules:
- I allow myself to take inspiration from my previous project and online resources, but no copy pasting AND NO AI.
- I can use crates like bootloader and x86_64, but I should try to understand the source of every function I use.

Some notes to myself:
- Add comments to every function I write (even if you think it's obvious, future you will thank present you)
- Write tests for every module I write
- Keep it modular
- Start simple, then add features incrementally
- HAVE FUN!
