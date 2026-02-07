use crate::mm::buddy::BuddyAllocator;
use crate::mm::slub::{PAGE_SIZE, PageProvider, SCache};
use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use spin::Mutex;
use x86_64::structures::paging::{PhysFrame, Size4KiB};

pub struct GlobalPageAllocator {
    frame_allocator: BuddyAllocator,
}

impl PageProvider for GlobalPageAllocator {
    fn alloc_page(&mut self) -> Option<*mut u8> {
        // We only support 4KiB pages for now (order 0)
        // TODO: support larger pages
        let frame = unsafe { self.frame_allocator.alloc(0) }?;
        // This should be a virtual address
        Some(frame)
    }

    fn free_page(&mut self, ptr: *mut u8) {
        unsafe { self.frame_allocator.dealloc(ptr, 0) };
    }
}

static PAGE_ALLOCATOR: Mutex<Option<GlobalPageAllocator>> = Mutex::new(None);

pub struct SlubAllocator {
    caches: [Mutex<SCache>; 8], // 16, 32, 64, 128, 256, 512, 1024, 2048
}

impl SlubAllocator {
    pub const fn new() -> Self {
        Self {
            caches: [
                Mutex::new(SCache::new(16)),
                Mutex::new(SCache::new(32)),
                Mutex::new(SCache::new(64)),
                Mutex::new(SCache::new(128)),
                Mutex::new(SCache::new(256)),
                Mutex::new(SCache::new(512)),
                Mutex::new(SCache::new(1024)),
                Mutex::new(SCache::new(2048)),
            ],
        }
    }
}

unsafe impl GlobalAlloc for SlubAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();

        // Handle large allocations (> 2048 bytes)
        if size > 2048 {
            // We only support single page allocations for large objects for now
            // TODO: Implement multi-page allocations
            if size <= PAGE_SIZE {
                let mut provider = PAGE_ALLOCATOR.lock();
                if let Some(p) = provider.as_mut() {
                    if let Some(ptr) = p.alloc_page() {
                        return ptr;
                    }
                }
            }
            return ptr::null_mut();
        }

        // Find index
        let index = if size <= 16 {
            0
        } else if size <= 32 {
            1
        } else if size <= 64 {
            2
        } else if size <= 128 {
            3
        } else if size <= 256 {
            4
        } else if size <= 512 {
            5
        } else if size <= 1024 {
            6
        } else {
            7
        };

        let mut cache = self.caches[index].lock();
        let mut provider = PAGE_ALLOCATOR.lock();
        if let Some(p) = provider.as_mut() {
            cache.alloc(p).unwrap_or(ptr::null_mut())
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size();
        if size > 2048 {
            let mut provider = PAGE_ALLOCATOR.lock();
            if let Some(p) = provider.as_mut() {
                p.free_page(ptr);
            }
            return;
        }

        let index = if size <= 16 {
            0
        } else if size <= 32 {
            1
        } else if size <= 64 {
            2
        } else if size <= 128 {
            3
        } else if size <= 256 {
            4
        } else if size <= 512 {
            5
        } else if size <= 1024 {
            6
        } else {
            7
        };

        let mut cache = self.caches[index].lock();
        let mut provider = PAGE_ALLOCATOR.lock();
        if let Some(p) = provider.as_mut() {
            unsafe { cache.dealloc(ptr, p) };
        }
    }
}

#[cfg(not(feature = "no_global_allocator"))] // Fixes issues with tests
#[global_allocator]
static ALLOCATOR: SlubAllocator = SlubAllocator::new();

pub fn init_heap(offset: usize) {
    let mut provider = PAGE_ALLOCATOR.lock();

    // Initialize directly in the Option to avoid stack overflow
    *provider = Some(GlobalPageAllocator {
        frame_allocator: BuddyAllocator::new(),
    });

    if let Some(p) = provider.as_mut() {
        p.frame_allocator.set_offset(offset);
    }
}

/// Add a physical frame to the buddy allocator
/// This should be called for each free frame detected during memory map parsing
///
/// # Safety
/// The caller must ensure that the provided frame is valid and not already in use, as this can lead to memory corruption if misused.
pub unsafe fn add_frame(start: *mut u8) {
    let mut provider = PAGE_ALLOCATOR.lock();
    if let Some(p) = provider.as_mut() {
        unsafe { p.frame_allocator.add_frame(start) };
    }
}

/// Allocate a physical frame from the buddy allocator
/// Returns the physical frame, or None if no frames are available
pub fn allocate_frame() -> Option<PhysFrame<Size4KiB>> {
    use x86_64::structures::paging::FrameAllocator;

    let mut provider = PAGE_ALLOCATOR.lock();
    if let Some(p) = provider.as_mut() {
        p.frame_allocator.allocate_frame()
    } else {
        None
    }
}
