use core::mem;
use core::ptr::{self, NonNull};

pub const PAGE_SIZE: usize = 4096; // 4KiB pages

/// Trait for providing pages
pub trait PageProvider {
    fn alloc_page(&mut self) -> Option<*mut u8>;
    fn free_page(&mut self, ptr: *mut u8);
}

/// Metadata stored at the beginning of every slab page.
pub struct SlabHeader {
    /// Pointer to the next slab in the partial list.
    next_slab: Option<NonNull<SlabHeader>>,
    /// Head of the free object list within this slab.
    freelist: Option<NonNull<FreeObject>>,
    /// Number of objects currently in use in this slab.
    in_use: usize,
}

/// A node in the free list, embedded in the free memory slots.
pub struct FreeObject {
    next: Option<NonNull<FreeObject>>,
}

/// A Slab Cache for a specific object size.
pub struct SCache {
    /// List of partial slabs (slabs with some free objects).
    partial: Option<NonNull<SlabHeader>>,
    /// Size of objects in this cache.
    size: usize,
}

unsafe impl Send for SCache {}

impl SCache {
    pub const fn new(size: usize) -> Self {
        Self {
            partial: None,
            size,
        }
    }

    pub fn alloc(&mut self, provider: &mut impl PageProvider) -> Option<*mut u8> {
        // 1. Check partial list
        if let Some(mut slab_ptr) = self.partial {
            let slab = unsafe { slab_ptr.as_mut() };

            // Take object from freelist
            if let Some(mut obj_ptr) = slab.freelist {
                let obj = unsafe { obj_ptr.as_mut() };
                slab.freelist = obj.next;
                slab.in_use += 1;

                // If slab is now full (no freelist), remove from partial
                if slab.freelist.is_none() {
                    self.partial = slab.next_slab;
                    slab.next_slab = None;
                }

                return Some(obj_ptr.as_ptr() as *mut u8);
            } else {
                // Should not happen if it's in partial list, unless logic error.
                // Remove from partial and try next.
                self.partial = slab.next_slab;
                return self.alloc(provider);
            }
        }

        // 2. No partial slabs, allocate new page
        let page_ptr = provider.alloc_page()?;

        // Initialize SlabHeader
        let slab_ptr = page_ptr as *mut SlabHeader;
        let header_size = mem::size_of::<SlabHeader>();

        // Align object start to the object size (simple alignment strategy)
        // Ensure we have space for header
        let mut object_start_offset = header_size;

        // Align up to self.size if it's a power of 2, or just ensure 8-byte alignment
        let align_mask = if self.size.is_power_of_two() {
            self.size - 1
        } else {
            7 // Default 8-byte alignment
        };

        object_start_offset = (object_start_offset + align_mask) & !align_mask;

        let object_start = unsafe { page_ptr.add(object_start_offset) };

        // Calculate capacity
        if object_start_offset >= PAGE_SIZE {
            return None;
        }
        let available_bytes = PAGE_SIZE - object_start_offset;
        let capacity = available_bytes / self.size;

        if capacity == 0 {
            return None;
        }

        // Initialize freelist in the page
        // We link them: 0 -> 1 -> 2 ... -> None
        let mut next_ptr: Option<NonNull<FreeObject>> = None;

        // Iterate backwards to build list so head is at index 0
        for i in (0..capacity).rev() {
            let offset = i * self.size;
            let ptr = unsafe { object_start.add(offset) } as *mut FreeObject;
            unsafe {
                (*ptr).next = next_ptr;
            }
            next_ptr = NonNull::new(ptr);
        }

        let mut slab = SlabHeader {
            next_slab: None,
            freelist: next_ptr,
            in_use: 0,
        };

        // We immediately allocate one object (the first one)
        let mut obj_ptr = slab.freelist.unwrap();
        let obj = unsafe { obj_ptr.as_mut() };
        slab.freelist = obj.next;
        slab.in_use = 1;

        // If there are still free objects, add to partial
        if slab.freelist.is_some() {
            slab.next_slab = self.partial;
            self.partial = NonNull::new(slab_ptr);
        }

        unsafe { ptr::write(slab_ptr, slab) };

        Some(obj_ptr.as_ptr() as *mut u8)
    }

    pub unsafe fn dealloc(&mut self, ptr: *mut u8, provider: &mut impl PageProvider) {
        // Find page start
        let page_ptr = (ptr as usize & !(PAGE_SIZE - 1)) as *mut u8;
        let slab_ptr = page_ptr as *mut SlabHeader;
        let slab = unsafe { &mut *slab_ptr };

        // Create FreeObject at ptr
        let obj_ptr = ptr as *mut FreeObject;
        unsafe { (*obj_ptr).next = slab.freelist };
        slab.freelist = NonNull::new(obj_ptr);
        slab.in_use -= 1;

        if slab.in_use == 0 {
            // Free the page
            self.remove_slab_from_partial(slab_ptr);
            provider.free_page(page_ptr);
        } else {
            // If it was full (not in partial) and now has 1 free, add to partial.
            // We check if it's in partial by checking if we just transitioned from full.
            // If `(*obj_ptr).next` (old freelist head) was None, it was full.
            if unsafe { (*obj_ptr).next.is_none() } {
                slab.next_slab = self.partial;
                self.partial = NonNull::new(slab_ptr);
            }
        }
    }

    fn remove_slab_from_partial(&mut self, slab_ptr: *mut SlabHeader) {
        let mut cur = &mut self.partial;
        while let Some(mut node) = *cur {
            if node.as_ptr() == slab_ptr {
                // Found it
                unsafe {
                    *cur = node.as_mut().next_slab;
                }
                return;
            }
            unsafe {
                cur = &mut node.as_mut().next_slab;
            }
        }
    }
}
