# Journal

## 19/12/2025 2:30h of work
I spent around 1h initializing everything and made a simple "Hello, World!" print to the serial port. It uses a UEFI bootloader and works pretty well so far (we have a wopping 50 lines of code so it would be hard to mess up). Next step is to set up a basic memory management system, refactor the serial driver a bit and find out why rust-analyzer is being weird.

Alright, I spent another 30 minutes fixing rust-analyzer (I forgot to add the kernel to the workspace memebers). I also split the print and exit drivers into their own modules.

I setup a basic memory mapper and allocator (I used the linked_list_allocator crate for now, I might write my own later). Everything seems to work fine, I can allocate memory and the heap is working. Next step is to implement the test runner and write some tests for the allocator.

Oh after 15 mins of questioning my life choices I found out that bootloader v0.11+ isn't compatible with the buildin rust test runner, so no tests for now. I'll try to write my own test runner later, seems like a fun challenge.