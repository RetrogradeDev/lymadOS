use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use x86_64::PhysAddr;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::page_table::FrameError;
use x86_64::structures::paging::{FrameAllocator, OffsetPageTable, PhysFrame, Size2MiB, Size4KiB};
use x86_64::{VirtAddr, structures::paging::PageTable};

/// Size constants
pub const PAGE_SIZE: u64 = 4096;
pub const HUGE_PAGE_SIZE: u64 = 2 * 1024 * 1024; // 2 MiB
pub const GIANT_PAGE_SIZE: u64 = 1024 * 1024 * 1024; // 1 GiB

/// Maximum number of free ranges we can track.
/// Starts as the number of usable regions from the bootloader memory map,
/// but can grow as allocations split ranges. 256 is very generous.
const MAX_RANGES: usize = 256;

/// Initialize a new OffsetPageTable.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
///
/// # Safety
/// The caller must ensure that the complete physical memory is mapped to virtual memory at the passed `physical_memory_offset`, and that this function is only called once during initialization to avoid undefined behavior.
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

/// Translates the given virtual address to the mapped physical address, or
/// `None` if the address is not mapped.
///
/// # Safety
/// The caller must ensure that the complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`.
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr> {
    translate_addr_inner(addr, physical_memory_offset)
}

/// Private function that is called by `translate_addr`.
///
/// This function is safe to limit the scope of `unsafe` because Rust treats
/// the whole body of unsafe functions as an unsafe block. This function must
/// only be reachable through `unsafe fn` from outside of this module.
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr> {
    // read the active level 4 frame from the CR3 register
    let (level_4_table_frame, _) = Cr3::read();

    let table_indexes = [
        addr.p4_index(),
        addr.p3_index(),
        addr.p2_index(),
        addr.p1_index(),
    ];
    let mut frame = level_4_table_frame;

    // traverse the multi-level page table
    for &index in &table_indexes {
        // convert the frame into a page table reference
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe { &*table_ptr };

        // read the page table entry and update `frame`
        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }

    // calculate the physical address by adding the page offset
    Some(frame.start_address() + u64::from(addr.page_offset()))
}

/// Represents a contiguous range of free physical memory
#[derive(Debug, Clone, Copy)]
struct PhysRange {
    start: u64,
    end: u64, // exclusive
}

impl PhysRange {
    const fn empty() -> Self {
        Self { start: 0, end: 0 }
    }

    fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }
}

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
/// Supports contiguous allocation and deallocation.
/// Uses a fixed-size array instead of Vec since this runs before the heap exists.
pub struct BootInfoFrameAllocator {
    /// Fixed-size array of free physical memory ranges
    free_ranges: [PhysRange; MAX_RANGES],
    /// Number of active ranges in the array
    range_count: usize,
    /// Total bytes allocated
    allocated_bytes: u64,
    /// Total bytes available at init
    total_bytes: u64,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// # Safety
    /// The caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        let mut free_ranges = [PhysRange::empty(); MAX_RANGES];
        let mut count = 0usize;
        let mut total_bytes = 0u64;

        for region in memory_map.iter() {
            if region.kind == MemoryRegionKind::Usable {
                // Align start up to page boundary
                let start = (region.start + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
                // Align end down to page boundary
                let end = region.end & !(PAGE_SIZE - 1);

                if end > start && count < MAX_RANGES {
                    free_ranges[count] = PhysRange::new(start, end);
                    count += 1;
                    total_bytes += end - start;
                }
            }
        }

        // Insertion sort by start address (small N, no alloc needed)
        for i in 1..count {
            let key = free_ranges[i];
            let mut j = i;
            while j > 0 && free_ranges[j - 1].start > key.start {
                free_ranges[j] = free_ranges[j - 1];
                j -= 1;
            }
            free_ranges[j] = key;
        }

        BootInfoFrameAllocator {
            free_ranges,
            range_count: count,
            allocated_bytes: 0,
            total_bytes,
        }
    }

    /// Returns the total amount of free memory in bytes
    pub fn free_memory(&self) -> u64 {
        self.total_bytes - self.allocated_bytes
    }

    /// Returns the total amount of allocated memory in bytes
    pub fn allocated_memory(&self) -> u64 {
        self.allocated_bytes
    }

    /// Returns the number of tracked free ranges
    pub fn range_count(&self) -> usize {
        self.range_count
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    pub fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> + '_ {
        self.free_ranges[..self.range_count]
            .iter()
            .flat_map(|range| {
                (range.start..range.end)
                    .step_by(PAGE_SIZE as usize)
                    .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
            })
    }

    /// Allocate `count` contiguous 4KiB frames.
    /// Returns the starting physical frame, or None if not enough contiguous memory.
    pub fn allocate_contiguous(&mut self, count: usize) -> Option<PhysFrame> {
        self.allocate_contiguous_aligned(count, PAGE_SIZE)
    }

    /// Allocate `count` contiguous 4KiB frames with a specific alignment.
    /// Returns the starting physical frame, or None if not enough contiguous memory.
    pub fn allocate_contiguous_aligned(
        &mut self,
        count: usize,
        alignment: u64,
    ) -> Option<PhysFrame> {
        if count == 0 {
            return None;
        }

        let required_size = count as u64 * PAGE_SIZE;

        // Find a suitable range
        for i in 0..self.range_count {
            let range = self.free_ranges[i];

            // Calculate aligned start within this range
            let aligned_start = (range.start + alignment - 1) & !(alignment - 1);

            // Check if the aligned allocation fits in this range
            if aligned_start >= range.start && aligned_start + required_size <= range.end {
                let alloc_end = aligned_start + required_size;

                // Determine how many new ranges we'll need (0, 1, or 2 pieces remain)
                let has_before = aligned_start > range.start;
                let has_after = alloc_end < range.end;
                let new_ranges_needed = has_before as usize + has_after as usize;

                // We're removing 1 range and potentially adding up to 2
                // Net change: new_ranges_needed - 1
                if new_ranges_needed > 1 && self.range_count >= MAX_RANGES {
                    // Would exceed capacity, try next range
                    continue;
                }

                // Remove the old range by shifting everything left
                self.remove_range(i);

                // Insert the remaining pieces
                if has_before {
                    self.insert_range_sorted(PhysRange::new(range.start, aligned_start));
                }
                if has_after {
                    self.insert_range_sorted(PhysRange::new(alloc_end, range.end));
                }

                self.allocated_bytes += required_size;

                return Some(PhysFrame::containing_address(PhysAddr::new(aligned_start)));
            }
        }

        None
    }

    /// Allocate a 2MiB huge page (properly aligned).
    pub fn allocate_huge_page(&mut self) -> Option<PhysFrame<Size2MiB>> {
        // Need 512 contiguous 4KiB pages, aligned to 2MiB
        let frame_4k = self.allocate_contiguous_aligned(512, HUGE_PAGE_SIZE)?;
        Some(PhysFrame::containing_address(frame_4k.start_address()))
    }

    /// Free `count` contiguous 4KiB frames starting at `frame`.
    ///
    /// # Safety
    /// The caller must ensure that the frames were previously allocated by this allocator
    /// and are no longer in use.
    pub unsafe fn free_contiguous(&mut self, frame: PhysFrame, count: usize) {
        if count == 0 {
            return;
        }

        let start = frame.start_address().as_u64();
        let end = start + count as u64 * PAGE_SIZE;

        self.allocated_bytes = self
            .allocated_bytes
            .saturating_sub(count as u64 * PAGE_SIZE);

        // Insert the freed range and coalesce
        self.insert_range_sorted(PhysRange::new(start, end));
        self.coalesce_ranges();
    }

    /// Free a single frame
    ///
    /// # Safety
    /// The caller must ensure that the frame was previously allocated by this allocator
    /// and is no longer in use.
    pub unsafe fn free_frame(&mut self, frame: PhysFrame) {
        // SAFETY: Caller guarantees the frame was allocated and is no longer in use
        unsafe { self.free_contiguous(frame, 1) };
    }

    /// Remove range at index, shifting remaining elements left
    fn remove_range(&mut self, index: usize) {
        for j in index..self.range_count - 1 {
            self.free_ranges[j] = self.free_ranges[j + 1];
        }
        self.range_count -= 1;
        self.free_ranges[self.range_count] = PhysRange::empty();
    }

    /// Insert a range in sorted order by start address
    fn insert_range_sorted(&mut self, range: PhysRange) {
        if self.range_count >= MAX_RANGES {
            panic!("BootInfoFrameAllocator: exceeded maximum number of free ranges");
        }

        // Find insertion point
        let mut pos = self.range_count;
        for i in 0..self.range_count {
            if range.start < self.free_ranges[i].start {
                pos = i;
                break;
            }
        }

        // Shift everything right to make room
        for j in (pos..self.range_count).rev() {
            self.free_ranges[j + 1] = self.free_ranges[j];
        }

        self.free_ranges[pos] = range;
        self.range_count += 1;
    }

    /// Merge adjacent free ranges
    fn coalesce_ranges(&mut self) {
        if self.range_count < 2 {
            return;
        }

        let mut i = 0;
        while i < self.range_count - 1 {
            if self.free_ranges[i].end == self.free_ranges[i + 1].start {
                // Merge: extend current range to cover the next one
                self.free_ranges[i].end = self.free_ranges[i + 1].end;
                self.remove_range(i + 1);
                // Don't increment i, check if we can merge more
            } else {
                i += 1;
            }
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.allocate_contiguous(1)
    }
}
