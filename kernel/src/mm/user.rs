use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB},
};

use crate::mm::allocator;

/// A wrapper that provides frames from the global buddy allocator
pub struct BuddyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for BuddyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        allocator::allocate_frame()
    }
}

/// Maps a new page at the given virtual address for userspace
///
/// Uses the buddy allocator to get a physical frame, then maps it
/// at the specified virtual address with the given flags.
///
/// Returns the physical address of the allocated frame so the caller
/// can write to it through the kernel's physical memory mapping.
pub fn map_user_page(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    vaddr: VirtAddr,
    flags: PageTableFlags,
) -> Result<PhysAddr, &'static str> {
    let page = Page::containing_address(vaddr);

    // 1. Allocate physical frame
    let frame = frame_allocator
        .allocate_frame()
        .ok_or("Failed to allocate frame")?;

    let phys_addr = frame.start_address();

    // 2. Map the page to the frame
    unsafe {
        mapper
            .map_to(page, frame, flags, frame_allocator)
            .map_err(|_| "Failed to map page")?
            .flush();
    }

    Ok(phys_addr)
}
