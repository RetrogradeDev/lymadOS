# Journal

## 19/12/2025 2:30h of work
I spent around 1h initializing everything and made a simple "Hello, World!" print to the serial port. It uses a UEFI bootloader and works pretty well so far (we have a wopping 50 lines of code so it would be hard to mess up). Next step is to set up a basic memory management system, refactor the serial driver a bit and find out why rust-analyzer is being weird.

Alright, I spent another 30 minutes fixing rust-analyzer (I forgot to add the kernel to the workspace memebers). I also split the print and exit drivers into their own modules.

I setup a basic memory mapper and allocator (I used the linked_list_allocator crate for now, I might write my own later). Everything seems to work fine, I can allocate memory and the heap is working. Next step is to implement the test runner and write some tests for the allocator.

Oh after 15 mins of questioning my life choices I found out that bootloader v0.11+ isn't compatible with the buildin rust test runner, so no tests for now. I'll try to write my own test runner later, seems like a fun challenge.

## 2/1/2026 Forgot to log my work of the last 3 days
I spent the last 3 days implementing a basic SLUB allocator, and a Buddy Frame Allocator. I learnt a lot about memory management in the process, and I think I have a good understanding of how it works now. They both worked fine the first try, which is nice but also a bit suspicious. I wrote some tests because of this, but they all passed, so I guess I'll find out later if something is broken. Next step will be using a bitmap in the frame allocator instead of a linked list, and then I can finally implement GDT, IDT and basic interrupts.

I spent an hour today refactoring the Buddy Allocator to use a bitmap and a double linked list. It was a bit tricky, but I think I got it working. I learnt creating a 32KB array on the stack is not a good idea, and horrible to debug.

Alright, I spent another hour implementing the GDT and TSS. It was pretty straightforward, I just followed Philip Oppermann's blog post on the topic. I also set up the stack for double faults, so that's nice. Next step is to implement a basic APIC timer, and keyboard interrupts.

## 3/1/2026 2:30h of work
I spent 2.5 hours today implementing a basic ACPI parser and APIC timer. It was a bit tricky, but I based a lot of the code on my previous OS project, so that helped. The timer seems to work fine, I can set up periodic interrupts and handle them. Next step is to implement keyboard interrupts.

# 4/1/2026 2h of work
I spent WAY too long debugging keyboard interrupts today. Turns out QEMU doesn't forward keyboard interrupts when using the "none" display option. After disabling that, everything worked fine, so I wasted more than an hour debugging this. I also implemented a basic keyboard driver that can read scancodes and print them. Next step is either some based event architecture to forward all these events (but I have to think about the design first), fix mouse interrupts or work on the userland.

# 6/1/2026 1h (today and yesterday combined)
I spent some time yesterday and today implementing a very basic event system. It ended up being just a global ArrayQueue, but it works fine for now. Next step is fixing mouse interrupts.

Alright, I spent 30 minutes figuring our how to enable the mouse by sending data over the ports, and then realised "ps2-mouse", the crate I ended up using to decode the mouse packets, already has an init function that does exactly that. We can now read mouse packets and add their states to the event queue. Next step is to implement a either a basic way of writing stuff to the screen, or the userland.

# 2/2/2026 3h (today and yesterday combined)
I spent some time yesterday and today implementing a basic userland switching, state saving and loading and syscalls. It was a bit tricky, but I think I got it working. I will first test it a bit more before writing about it in detail. After that, I'll think really hard about a good design for driver and syscall handling. I was thinking about making it really modular, so drivers can specify what other drivers they depend on, and subscribe and push events.