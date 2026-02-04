use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB},
};

/// Maps a new page at the given virtual address for userspace
///
/// Uses the buddy allocator to get a physical frame, then maps it
/// at the specified virtual address with the given flags.
pub fn map_user_page(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    vaddr: VirtAddr,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    let page = Page::containing_address(vaddr);

    // 1. Allocate physical frame
    let frame = frame_allocator
        .allocate_frame()
        .ok_or("Failed to allocate frame")?;

    // 2. Map the page to the frame
    unsafe {
        mapper
            .map_to(page, frame, flags, frame_allocator)
            .map_err(|_| "Failed to map page")?
            .flush();
    }

    Ok(())
}
