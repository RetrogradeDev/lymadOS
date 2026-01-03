# Journal

## 19/12/2025 2:30h of work
I spent around 1h initializing everything and made a simple "Hello, World!" print to the serial port. It uses a UEFI bootloader and works pretty well so far (we have a wopping 50 lines of code so it would be hard to mess up). Next step is to set up a basic memory management system, refactor the serial driver a bit and find out why rust-analyzer is being weird.

Alright, I spent another 30 minutes fixing rust-analyzer (I forgot to add the kernel to the workspace memebers). I also split the print and exit drivers into their own modules.

I setup a basic memory mapper and allocator (I used the linked_list_allocator crate for now, I might write my own later). Everything seems to work fine, I can allocate memory and the heap is working. Next step is to implement the test runner and write some tests for the allocator.

Oh after 15 mins of questioning my life choices I found out that bootloader v0.11+ isn't compatible with the buildin rust test runner, so no tests for now. I'll try to write my own test runner later, seems like a fun challenge.

## 2/1/2026 Forgot to log my work of the last 3 days
I spent the last 3 days implementing a basic SLUB allocator, and a Buddy Frame Allocator. I learnt a lot about memory management in the process, and I think I have a good understanding of how it works now. They both worked fine the first try, which is nice but also a bit suspicious. I wrote some tests because of this, but they all passed, so I guess I'll find out later if something is broken. Next step will be using a bitmap in the frame allocator instead of a linked list, and then I can finally implement GDT, IDT and basic interrupts.

I spent an hour today refactoring the Buddy Allocator to use a bitmap and a double linked list. It was a bit tricky, but I think I got it working. I learnt creating a 32KB array on the stack is not a good idea, and horrible to debug.