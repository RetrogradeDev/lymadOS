use core::ptr::NonNull;

const MAX_ORDER: usize = 12; // Supports up to 2^12 * 4KiB = 16MiB blocks

pub struct BuddyAllocator {
    // Heads of the free lists for each order
    // free_lists[0] -> order 0 (4KiB)
    // free_lists[1] -> order 1 (8KiB), etc.
    free_lists: [Option<NonNull<FreeFrame>>; MAX_ORDER],
}

// Stored in the free physcal memory itself
#[repr(C)]
struct FreeFrame {
    next: Option<NonNull<FreeFrame>>,
}

impl BuddyAllocator {
    pub const fn new() -> Self {
        Self {
            free_lists: [None; MAX_ORDER],
        }
    }

    fn calculate_buddy_address(ptr: *mut u8, order: usize) -> *mut u8 {
        let block_size = 1 << order; // Size in pages
        let addr = ptr as usize;
        // XOR toggles the bit corresponding to the block size
        let buddy_addr = addr ^ (block_size * 4096);
        buddy_addr as *mut u8
    }

    // Allocates a block of memory
    // Returns a pointer to the start of the block
    pub unsafe fn alloc(&mut self, order: usize) -> Option<*mut u8> {
        if order >= MAX_ORDER {
            return None;
        }

        // First, check free_lists[order] for a free block
        // If found, remove and return it
        if let Some(frame_ptr) = self.free_lists[order] {
            let frame = unsafe { frame_ptr.as_ref() };
            self.free_lists[order] = frame.next;
            return Some(frame_ptr.as_ptr() as *mut u8);
        }

        // If empty, try alloc(order + 1) and divide it into two buddies
        if order + 1 < MAX_ORDER {
            if let Some(ptr) = unsafe { self.alloc(order + 1) } {
                let buddy_addr = Self::calculate_buddy_address(ptr, order);

                // Add the second buddy to the free list
                self.push_free(buddy_addr, order);

                return Some(ptr);
            }
        }
        None
    }

    // Deallocates a block of memory
    pub unsafe fn dealloc(&mut self, ptr: *mut u8, order: usize) {
        // If we are at max order, just add to free list
        if order >= MAX_ORDER - 1 {
            self.push_free(ptr, order);
            return;
        }

        // Calculate buddy address
        let buddy_addr = Self::calculate_buddy_address(ptr, order);

        // Check if buddy is free
        // TODO: Maintain a more efficient way to find buddies, like a bitmap or tree structure
        // For now, we'll just walk the list
        let mut prev = None;
        let mut curr = self.free_lists[order];
        while let Some(node) = curr {
            if node.as_ptr() as *mut u8 == buddy_addr {
                break;
            }
            prev = curr;
            curr = unsafe { node.as_ref().next };
        }

        // If buddy found, remove it from free list and merge
        if let Some(buddy_ptr) = curr {
            // Remove buddy from free list
            if let Some(mut prev_ptr) = prev {
                unsafe {
                    prev_ptr.as_mut().next = buddy_ptr.as_ref().next;
                }
            } else {
                self.free_lists[order] = unsafe { buddy_ptr.as_ref().next };
            }

            // Calculate new block address
            let merged_addr = if ptr < buddy_addr { ptr } else { buddy_addr };

            // Recursively dealloc merged block at higher order
            unsafe { self.dealloc(merged_addr, order + 1) };
        } else {
            // Buddy not found, just add this block to free list
            self.push_free(ptr, order);
        }
    }

    /// Adds a free frame (order 0) to the allocator.
    /// This is used during initialization to feed memory into the system.
    pub unsafe fn add_frame(&mut self, frame: *mut u8) {
        // We treat incoming frames as Order 0 (4KiB)
        // The dealloc logic will automatically merge them into larger blocks if their buddies are present.
        unsafe { self.dealloc(frame, 0) };
    }

    fn push_free(&mut self, ptr: *mut u8, order: usize) {
        let node_ptr = ptr as *mut FreeFrame;
        // Initialize the node in memory
        unsafe {
            (*node_ptr).next = self.free_lists[order];
        }
        // Update head
        self.free_lists[order] = NonNull::new(node_ptr);
    }
}

unsafe impl Send for BuddyAllocator {}
