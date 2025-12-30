use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self, NonNull};
use spin::Mutex; // You'll need the 'spin' crate for kernel synchronization

const PAGE_SIZE: usize = 4096;
const PAGE_MASK: usize = !(PAGE_SIZE - 1);

/// The intrusive node stored inside free memory blocks.
struct Node {
    next: Option<NonNull<Node>>,
}

/// Metadata stored at the very beginning of every 4KB page.
struct SlabHeader {
    next_slab: Option<NonNull<SlabHeader>>,
    free_list: Option<NonNull<Node>>,
    object_size: usize,
    allocated_count: usize,
    max_objects: usize,
}

impl SlabHeader {
    /// Initialize a page as a new Slab.
    unsafe fn init(ptr: usize, object_size: usize) -> &'static mut Self {
        let header_size = core::mem::size_of::<SlabHeader>();
        // Calculate how many objects fit after the header
        let max_objects = (PAGE_SIZE - header_size) / object_size;

        let header = unsafe { &mut *(ptr as *mut SlabHeader) };
        header.next_slab = None;
        header.free_list = None;
        header.object_size = object_size;
        header.allocated_count = 0;
        header.max_objects = max_objects;

        // Build the intrusive free list for all slots in this page
        for i in 0..max_objects {
            let slot_ptr = (ptr + header_size + (i * object_size)) as *mut Node;
            unsafe { header.push_node(slot_ptr) };
        }
        header
    }

    unsafe fn push_node(&mut self, ptr: *mut Node) {
        unsafe {
            (*ptr).next = self.free_list;
        }
        self.free_list = Some(unsafe { NonNull::new_unchecked(ptr) });
    }

    unsafe fn pop_node(&mut self) -> *mut u8 {
        let node = self.free_list.take().unwrap();
        self.free_list = unsafe { node.as_ref().next };
        self.allocated_count += 1;
        node.as_ptr() as *mut u8
    }
}

struct KmemCache {
    object_size: usize,
    partial_slabs: Option<NonNull<SlabHeader>>,
}

impl KmemCache {
    pub const fn new(size: usize) -> Self {
        Self {
            object_size: size,
            partial_slabs: None,
        }
    }

    pub unsafe fn alloc<F>(&mut self, allocate_frame: F) -> *mut u8
    where
        F: Fn() -> Option<*mut u8>,
    {
        // 1. Try to get a slot from existing partial slabs
        if let Some(mut slab_ptr) = self.partial_slabs {
            let slab = unsafe { slab_ptr.as_mut() };
            let ptr = unsafe { slab.pop_node() };

            // If this slab is now full, remove it from the partial list
            if slab.allocated_count == slab.max_objects {
                self.partial_slabs = slab.next_slab;
            }
            return ptr;
        }

        // 2. No partial slabs available. Request a new 4KB page from the Frame Allocator.
        if let Some(page_ptr) = allocate_frame() {
            let slab = unsafe { SlabHeader::init(page_ptr as usize, self.object_size) };
            let ptr = unsafe { slab.pop_node() };

            // Link this new slab into our partial list
            slab.next_slab = self.partial_slabs;
            self.partial_slabs = Some(unsafe { NonNull::new_unchecked(slab) });

            return ptr;
        }

        ptr::null_mut()
    }

    pub unsafe fn dealloc(&mut self, ptr: *mut u8) {
        // BITMASK TRICK: Find the header by zeroing the lower 12 bits
        let header_ptr = (ptr as usize & PAGE_MASK) as *mut SlabHeader;
        let slab = unsafe { &mut *header_ptr };

        let was_full = slab.allocated_count == slab.max_objects;
        unsafe { slab.push_node(ptr as *mut Node) };
        slab.allocated_count -= 1;

        // If it was full, it's now partial, so re-add it to the list
        if was_full {
            slab.next_slab = self.partial_slabs;
            self.partial_slabs = Some(unsafe { NonNull::new_unchecked(slab) });
        }

        // TODO: If allocated_count == 0, remove it from the list and free the 4KB frame back to the OS.
    }
}

// The mutex should be safe to send between threads
unsafe impl Sync for SlabHeader {}
unsafe impl Send for SlabHeader {}
unsafe impl Sync for KmemCache {}
unsafe impl Send for KmemCache {}

pub struct SlubAllocator {
    // Buckets for sizes: 8, 16, 32, 64, 128, 256, 512, 1024, 2048
    buckets: [Mutex<KmemCache>; 9],
}

impl SlubAllocator {
    pub const fn new() -> Self {
        Self {
            buckets: [
                Mutex::new(KmemCache::new(8)),
                Mutex::new(KmemCache::new(16)),
                Mutex::new(KmemCache::new(32)),
                Mutex::new(KmemCache::new(64)),
                Mutex::new(KmemCache::new(128)),
                Mutex::new(KmemCache::new(256)),
                Mutex::new(KmemCache::new(512)),
                Mutex::new(KmemCache::new(1024)),
                Mutex::new(KmemCache::new(2048)),
            ],
        }
    }

    fn get_bucket(&self, size: usize) -> Option<&Mutex<KmemCache>> {
        let idx = match size {
            0..=8 => 0,
            9..=16 => 1,
            17..=32 => 2,
            33..=64 => 3,
            65..=128 => 4,
            129..=256 => 5,
            257..=512 => 6,
            513..=1024 => 7,
            1025..=2048 => 8,
            _ => return None,
        };
        Some(&self.buckets[idx])
    }
}

unsafe impl GlobalAlloc for SlubAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(layout.align());

        if let Some(bucket) = self.get_bucket(size) {
            let mut cache = bucket.lock();
            unsafe {
                cache.alloc(|| {
                    // TODO: FRAME_ALLOCATOR.alloc_page()
                    None
                })
            }
        } else {
            // Layout is too big (> 2048), bypass SLUB and go to Frame Allocator
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size().max(layout.align());
        if let Some(bucket) = self.get_bucket(size) {
            let mut cache = bucket.lock();
            unsafe { cache.dealloc(ptr) };
        } else {
            // Free huge layout via Frame Allocator
        }
    }
}

#[global_allocator]
static SLUB_ALLOCATOR: SlubAllocator = SlubAllocator::new();
