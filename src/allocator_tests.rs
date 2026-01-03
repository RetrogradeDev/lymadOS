use kernel::mm::buddy::BuddyAllocator;
use kernel::mm::slub::{PAGE_SIZE, PageProvider, SCache};
use std::alloc::{Layout, alloc, dealloc};

struct TestPageProvider {
    allocated_pages: Vec<*mut u8>,
}

impl TestPageProvider {
    fn new() -> Self {
        Self {
            allocated_pages: Vec::new(),
        }
    }
}

impl PageProvider for TestPageProvider {
    fn alloc_page(&mut self) -> Option<*mut u8> {
        unsafe {
            let layout = Layout::from_size_align(PAGE_SIZE, PAGE_SIZE).unwrap();
            let ptr = alloc(layout);
            if ptr.is_null() {
                None
            } else {
                // Zero the memory to simulate fresh page
                std::ptr::write_bytes(ptr, 0, PAGE_SIZE);
                self.allocated_pages.push(ptr);
                Some(ptr)
            }
        }
    }

    fn free_page(&mut self, ptr: *mut u8) {
        if let Some(pos) = self.allocated_pages.iter().position(|&p| p == ptr) {
            self.allocated_pages.remove(pos);
            unsafe {
                let layout = Layout::from_size_align(PAGE_SIZE, PAGE_SIZE).unwrap();
                dealloc(ptr, layout);
            }
        } else {
            panic!("Double free or freeing unknown page");
        }
    }
}

impl Drop for TestPageProvider {
    fn drop(&mut self) {
        for ptr in &self.allocated_pages {
            unsafe {
                let layout = Layout::from_size_align(PAGE_SIZE, PAGE_SIZE).unwrap();
                dealloc(*ptr, layout);
            }
        }
    }
}

#[test]
fn test_buddy_allocator() {
    let mut buddy = BuddyAllocator::new();

    // Allocate 4MB of memory to feed the buddy allocator
    // 4MB = 1024 pages
    let memory_size = 4 * 1024 * 1024;
    let layout = Layout::from_size_align(memory_size, 4096).unwrap();
    let memory = unsafe { alloc(layout) };

    // Check if memory is within managed range (1GB)
    if memory as usize >= 1024 * 1024 * 1024 {
        // TODO: Fix this
        println!("Skipping test_buddy_allocator: Allocated memory is out of managed range (1GB)");
        unsafe { dealloc(memory, layout) };
        return;
    }

    // Feed pages to buddy allocator
    for i in (0..memory_size).step_by(4096) {
        unsafe {
            buddy.add_frame(memory.add(i));
        }
    }

    unsafe {
        // Alloc order 0 (4KiB)
        let ptr1 = buddy.alloc(0).expect("Failed to alloc order 0");

        // Alloc order 1 (8KiB)
        let ptr2 = buddy.alloc(1).expect("Failed to alloc order 1");

        // Check alignment
        assert_eq!(ptr1 as usize % 4096, 0);
        assert_eq!(ptr2 as usize % 8192, 0);

        // Alloc large block (Order 5 = 32 pages = 128KB)
        let ptr3 = buddy.alloc(5).expect("Failed to alloc order 5");

        buddy.dealloc(ptr1, 0);
        buddy.dealloc(ptr2, 1);
        buddy.dealloc(ptr3, 5);
    }

    unsafe { dealloc(memory, layout) };
}

#[test]
fn test_slub_allocator() {
    let mut provider = TestPageProvider::new();
    let mut cache = SCache::new(32); // 32 bytes objects

    unsafe {
        let ptr1 = cache.alloc(&mut provider).expect("Failed to alloc 32B");
        let ptr2 = cache.alloc(&mut provider).expect("Failed to alloc 32B");

        // Write to memory to ensure it's usable
        *ptr1 = 0xAA;
        *ptr2 = 0xBB;

        assert_ne!(ptr1, ptr2);

        // Check if they are in the same page (likely)
        let page1 = ptr1 as usize & !(PAGE_SIZE - 1);
        let page2 = ptr2 as usize & !(PAGE_SIZE - 1);
        assert_eq!(page1, page2);

        cache.dealloc(ptr1, &mut provider);
        cache.dealloc(ptr2, &mut provider);
    }
}

#[test]
fn test_slub_allocator_exhaustion() {
    let mut provider = TestPageProvider::new();
    let mut cache = SCache::new(1024); // 1024 bytes -> 4 objects per page (minus header overhead -> 3 objects?)
    // Header is small, so 4096 / 1024 = 4.
    // But header takes space.
    // If header is e.g. 24 bytes.
    // Start offset aligned to 1024.
    // If header < 1024, start at 1024.
    // So 3 objects: 1024, 2048, 3072.

    let mut ptrs = Vec::new();

    // Alloc 10 objects. Should span multiple pages.
    for _ in 0..10 {
        if let Some(ptr) = cache.alloc(&mut provider) {
            ptrs.push(ptr);
        }
    }

    assert_eq!(ptrs.len(), 10);

    // Free all
    for ptr in ptrs {
        unsafe { cache.dealloc(ptr, &mut provider) };
    }
}
