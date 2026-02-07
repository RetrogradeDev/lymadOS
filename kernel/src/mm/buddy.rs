use core::ptr::NonNull;

use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

const MAX_ORDER: usize = 12;
const PAGE_SIZE: usize = 4096;
// 1GB RAM / 4KiB pages = 262,144 pages
const MAX_PAGES: usize = 262_144;
// We need 1 bit per pair of buddies.
// Order 0: 131,072 pairs (131,072 bits)
// Order 1: 65,536 pairs (65,536 bits)
// ...
// Total bits < MAX_PAGES.
// 262,144 bits / 8 = 32,768 bytes.
const BITMAP_SIZE: usize = MAX_PAGES / 8;

static mut BITMAP_STORAGE: [u8; BITMAP_SIZE] = [0; BITMAP_SIZE];

pub struct BuddyAllocator {
    // Heads of the free lists for each order
    // free_lists[0] -> order 0 (4KiB)
    // free_lists[1] -> order 1 (8KiB), etc.
    free_lists: [Option<NonNull<FreeFrame>>; MAX_ORDER],
    // Bitmap to track the state of buddy pairs
    // 0: Both buddies are in the same state (both free or both used)
    // 1: One buddy is free, one is used
    bitmap: &'static mut [u8],
    // Virtual memory offset (phys_mem_offset)
    offset: usize,
}

#[repr(C)]
struct FreeFrame {
    next: Option<NonNull<FreeFrame>>,
    prev: Option<NonNull<FreeFrame>>,
}

impl BuddyAllocator {
    pub fn new() -> Self {
        Self {
            free_lists: [None; MAX_ORDER],
            bitmap: unsafe { &mut *core::ptr::addr_of_mut!(BITMAP_STORAGE) },
            offset: 0,
        }
    }

    pub fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }

    /// Calculates the index of the bit corresponding to the pair of buddies
    /// for a given page index and order.
    fn get_bit_index(&self, page_idx: usize, order: usize) -> usize {
        // Calculate offset for this order in the bitmap
        // Offset = Sum(N / 2^(i+1)) for i from 0 to order-1
        let mut offset = 0;
        for i in 0..order {
            offset += MAX_PAGES >> (i + 1);
        }

        // The pair index within this order is page_idx / 2^(order+1)
        // We use >> (order + 1)
        offset + (page_idx >> (order + 1))
    }

    /// Toggles the bit for the given page and order.
    /// Returns the new value of the bit (true = 1, false = 0).
    fn toggle_bit(&mut self, page_idx: usize, order: usize) -> bool {
        let bit_idx = self.get_bit_index(page_idx, order);
        let byte_idx = bit_idx / 8;
        let bit_offset = bit_idx % 8;

        self.bitmap[byte_idx] ^= 1 << bit_offset;
        (self.bitmap[byte_idx] & (1 << bit_offset)) != 0
    }

    fn calculate_buddy_address(&self, ptr: *mut u8, order: usize) -> *mut u8 {
        let block_size = 1 << order; // Size in pages
        let addr = ptr as usize;
        // Convert to relative address (physical-like)
        let relative_addr = addr - self.offset;
        // XOR toggles the bit corresponding to the block size
        let buddy_relative_addr = relative_addr ^ (block_size * PAGE_SIZE);
        // Convert back to virtual address
        (buddy_relative_addr + self.offset) as *mut u8
    }

    // Allocates a block of memory
    // Returns a pointer to the start of the block
    //
    // # Safety
    // The caller must ensure that the returned pointer is used correctly and that the order is valid
    pub unsafe fn alloc(&mut self, order: usize) -> Option<*mut u8> {
        if order >= MAX_ORDER {
            return None;
        }

        // Try to find a free block at the requested order
        if let Some(frame_ptr) = self.free_lists[order] {
            // Remove from free list
            unsafe { self.remove_frame(frame_ptr.as_ptr() as *mut u8, order) };

            // Toggle bit. Since we are allocating one of a pair, and the other is presumably used
            // (otherwise they would be merged), the bit should go from 1 -> 0.
            // We only track bits for orders < MAX_ORDER - 1
            if order < MAX_ORDER - 1 {
                let page_idx = (frame_ptr.as_ptr() as usize - self.offset) / PAGE_SIZE;
                self.toggle_bit(page_idx, order);
            }

            return Some(frame_ptr.as_ptr() as *mut u8);
        }

        // If no free block, try to split a larger block
        if let Some(ptr) = unsafe { self.alloc(order + 1) } {
            let buddy_addr = self.calculate_buddy_address(ptr, order);

            // We have a block of order+1. We split it into two blocks of order.
            // We return `ptr` and free `buddy_addr`.
            // The pair (ptr, buddy) is now "One used, one free".
            // The bit should become 1.
            if order < MAX_ORDER - 1 {
                let page_idx = (ptr as usize - self.offset) / PAGE_SIZE;
                self.toggle_bit(page_idx, order);
            }

            // Add the buddy to the free list
            unsafe { self.push_free(buddy_addr, order) };

            return Some(ptr);
        }

        None
    }

    // Deallocates a block of memory
    //
    // # Safety
    // The caller must ensure that the pointer and order are valid and that the block was previously allocated, as misuse can lead to memory corruption.
    pub unsafe fn dealloc(&mut self, ptr: *mut u8, order: usize) {
        let addr = ptr as usize;
        if addr < self.offset || addr >= self.offset + MAX_PAGES * PAGE_SIZE {
            // Address out of managed range
            return;
        }

        // If we are at the max order, we can't merge further
        if order >= MAX_ORDER - 1 {
            unsafe { self.push_free(ptr, order) };
            return;
        }

        let page_idx = (ptr as usize - self.offset) / PAGE_SIZE;

        // Toggle bit for this pair
        let is_now_one = self.toggle_bit(page_idx, order);

        if is_now_one {
            // Bit became 1. This means the state is now "One free, one used".
            // So we cannot merge. Just add to free list.
            unsafe { self.push_free(ptr, order) };
        } else {
            // Bit became 0. This means the state is now "Both free" (since we just freed one).
            // We must merge.
            let buddy_addr = self.calculate_buddy_address(ptr, order);

            // Remove buddy from free list
            // Note: We don't need to remove `ptr` because it wasn't in the list yet.
            unsafe { self.remove_frame(buddy_addr, order) };

            // Merge and recurse
            let merged_addr = if ptr < buddy_addr { ptr } else { buddy_addr };
            unsafe { self.dealloc(merged_addr, order + 1) };
        }
    }

    /// Adds a free frame (order 0) to the allocator.
    /// This is used during initialization to feed memory into the system.
    ///
    /// # Safety
    /// The caller must ensure that the provided frame is valid and not already in use, as this can lead to memory corruption if misused.
    pub unsafe fn add_frame(&mut self, frame: *mut u8) {
        let addr = frame as usize;
        if addr < self.offset || addr >= self.offset + MAX_PAGES * PAGE_SIZE {
            return;
        }
        unsafe { self.dealloc(frame, 0) };
    }

    unsafe fn push_free(&mut self, ptr: *mut u8, order: usize) {
        let frame_ptr = ptr as *mut FreeFrame;
        let frame = unsafe { &mut *frame_ptr };

        frame.prev = None;
        frame.next = self.free_lists[order];

        if let Some(mut head) = self.free_lists[order] {
            unsafe { head.as_mut().prev = NonNull::new(frame_ptr) };
        }

        self.free_lists[order] = NonNull::new(frame_ptr);
    }

    unsafe fn remove_frame(&mut self, ptr: *mut u8, order: usize) {
        let frame_ptr = ptr as *mut FreeFrame;
        // We assume ptr is valid and in the list because the bitmap said so
        let frame = unsafe { &mut *frame_ptr };

        if let Some(mut prev) = frame.prev {
            unsafe { prev.as_mut().next = frame.next };
        } else {
            self.free_lists[order] = frame.next;
        }

        if let Some(mut next) = frame.next {
            unsafe { next.as_mut().prev = frame.prev };
        }

        // Clean up pointers
        frame.next = None;
        frame.prev = None;
    }
}

unsafe impl Send for BuddyAllocator {}

unsafe impl FrameAllocator<Size4KiB> for BuddyAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        unsafe { self.alloc(0) }.map(|ptr| {
            let phys_addr = (ptr as usize - self.offset) as u64;
            PhysFrame::containing_address(x86_64::PhysAddr::new(phys_addr))
        })
    }
}
