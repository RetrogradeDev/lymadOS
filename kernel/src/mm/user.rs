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

/// Set USER_ACCESSIBLE flag on all page table levels for a given page
/// probably the ugliest and most inefficient code ever but if it works, don't touch it
pub unsafe fn set_page_user_accessible(
    mapper: &mut x86_64::structures::paging::OffsetPageTable,
    page: x86_64::structures::paging::Page<x86_64::structures::paging::Size4KiB>,
    writable: bool,
    executable: bool,
) {
    use x86_64::registers::control::Cr3;
    use x86_64::structures::paging::{PageTable, PageTableFlags};

    let virt = page.start_address();
    let phys_offset = mapper.phys_offset();

    let (l4_frame, _) = Cr3::read();
    let l4_table: &mut PageTable =
        unsafe { &mut *(phys_offset + l4_frame.start_address().as_u64()).as_mut_ptr() };

    let l4_entry = &mut l4_table[virt.p4_index()];
    l4_entry.set_flags(l4_entry.flags() | PageTableFlags::USER_ACCESSIBLE);

    let l3_frame = l4_entry.frame().expect("L4 entry not present");
    let l3_table: &mut PageTable =
        unsafe { &mut *(phys_offset + l3_frame.start_address().as_u64()).as_mut_ptr() };
    let l3_entry = &mut l3_table[virt.p3_index()];
    l3_entry.set_flags(l3_entry.flags() | PageTableFlags::USER_ACCESSIBLE);

    let l2_frame = l3_entry.frame().expect("L3 entry not present");
    let l2_table: &mut PageTable =
        unsafe { &mut *(phys_offset + l2_frame.start_address().as_u64()).as_mut_ptr() };
    let l2_entry = &mut l2_table[virt.p2_index()];

    if l2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
        let mut new_flags = l2_entry.flags() | PageTableFlags::USER_ACCESSIBLE;
        if writable {
            new_flags |= PageTableFlags::WRITABLE;
        }
        if executable {
            new_flags &= !PageTableFlags::NO_EXECUTE;
        }
        l2_entry.set_flags(new_flags);
    } else {
        l2_entry.set_flags(l2_entry.flags() | PageTableFlags::USER_ACCESSIBLE);

        let l1_frame = l2_entry.frame().expect("L2 entry not present");
        let l1_table: &mut PageTable =
            unsafe { &mut *(phys_offset + l1_frame.start_address().as_u64()).as_mut_ptr() };
        let l1_entry = &mut l1_table[virt.p1_index()];

        let mut new_flags = l1_entry.flags() | PageTableFlags::USER_ACCESSIBLE;
        if writable {
            new_flags |= PageTableFlags::WRITABLE;
        }
        if executable {
            new_flags &= !PageTableFlags::NO_EXECUTE;
        }
        l1_entry.set_flags(new_flags);
    }

    x86_64::instructions::tlb::flush(virt);
}
