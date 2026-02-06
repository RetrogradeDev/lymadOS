// Elf parser and loader

use goblin::elf64::header::Header;
use goblin::elf64::program_header::ProgramHeader;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, PageTableFlags, Size4KiB},
};

use crate::{mm::user::map_user_page, serial_println};

/// User stack is placed at a fixed address below the kernel
/// Stack grows downward, so this is the top of the stack
pub const USER_STACK_TOP: u64 = 0x7FFFFF000;
/// Size of user stack: 16 pages = 64 KiB
pub const USER_STACK_PAGES: u64 = 16;
pub const USER_STACK_SIZE: u64 = USER_STACK_PAGES * 4096;

#[derive(Debug)]
pub enum Error {
    MappingFailed(&'static str),
    InvalidElf(goblin::error::Error),
}

/// Result of loading an ELF: entry point and stack pointer
pub struct ElfLoadResult {
    pub entry_point: u64,
    pub stack_top: u64,
}

/// Load an ELF binary into memory and allocate a user stack
///
/// `phys_mem_offset` is used to write to physical frames through the kernel's
/// identity-mapped physical memory region.
///
/// Returns the entry point address and stack top pointer
pub fn load_elf(
    data: &[u8],
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    phys_mem_offset: VirtAddr,
) -> Result<ElfLoadResult, Error> {
    serial_println!("load_elf: data len={}", data.len());

    // Parse ELF header directly (no allocation)
    if data.len() < core::mem::size_of::<Header>() {
        return Err(Error::MappingFailed("ELF too small for header"));
    }

    let header: &Header = unsafe { &*(data.as_ptr() as *const Header) };

    // Validate ELF magic
    if &header.e_ident[0..4] != b"\x7fELF" {
        return Err(Error::MappingFailed("Invalid ELF magic"));
    }

    // Check 64-bit
    if header.e_ident[4] != 2 {
        return Err(Error::MappingFailed("Not a 64-bit ELF"));
    }

    let entry = header.e_entry;
    let ph_offset = header.e_phoff as usize;
    let ph_count = header.e_phnum as usize;
    let ph_size = header.e_phentsize as usize;

    serial_println!(
        "Loading ELF: entry=0x{:x}, {} program headers",
        entry,
        ph_count
    );

    // Process each program header
    serial_println!(
        "About to process {} program headers, ph_offset={}, ph_size={}",
        ph_count,
        ph_offset,
        ph_size
    );

    for i in 0..ph_count {
        serial_println!("  Processing PH[{}]...", i);

        let ph_start = ph_offset + i * ph_size;
        serial_println!("    ph_start={}", ph_start);

        if ph_start + core::mem::size_of::<ProgramHeader>() > data.len() {
            return Err(Error::MappingFailed("Program header out of bounds"));
        }

        serial_println!("    Reading PH struct...");
        let ph_ptr = data.as_ptr();
        let ph_ptr_offset = unsafe { ph_ptr.add(ph_start) };
        let ph: &ProgramHeader = unsafe { &*(ph_ptr_offset as *const ProgramHeader) };
        serial_println!("    Read complete, type={}", ph.p_type);

        // PT_LOAD = 1
        if ph.p_type == 1 {
            serial_println!("    LOAD segment");

            let vaddr_start = ph.p_vaddr;
            serial_println!("    vaddr_start=0x{:x}", vaddr_start);
            let memsz = ph.p_memsz;
            let filesz = ph.p_filesz;
            let offset = ph.p_offset;
            let flags = ph.p_flags;

            serial_println!(
                "  LOAD: vaddr=0x{:x}, memsz=0x{:x}, filesz=0x{:x}, flags=0x{:x}",
                vaddr_start,
                memsz,
                filesz,
                flags
            );

            // Determine page flags
            // PF_W = 2, PF_X = 1
            // NOTE: We always map as writable initially so we can copy data,
            // then we'll need to remap with proper flags later // TODO
            let mut page_flags = PageTableFlags::PRESENT
                | PageTableFlags::USER_ACCESSIBLE
                | PageTableFlags::WRITABLE; // Always writable for now to allow copy
            if flags & 1 == 0 {
                page_flags |= PageTableFlags::NO_EXECUTE;
            }

            // Map all pages for this segment and copy data through physical memory mapping
            let start_page = vaddr_start & !0xFFF;
            let end_page = (vaddr_start + memsz + 0xFFF) & !0xFFF;

            serial_println!("    Mapping pages 0x{:x} - 0x{:x}", start_page, end_page);

            // For each page, map it and copy the relevant portion of the segment
            for page_vaddr in (start_page..end_page).step_by(4096) {
                serial_println!("    Mapping page 0x{:x}", page_vaddr);

                // Map the page and get its physical address
                let phys_addr = map_user_page(
                    mapper,
                    frame_allocator,
                    VirtAddr::new(page_vaddr),
                    page_flags,
                )
                .map_err(|e| Error::MappingFailed(e))?;

                // Calculate kernel-accessible address for this physical frame
                let kernel_ptr = (phys_mem_offset.as_u64() + phys_addr.as_u64()) as *mut u8;

                // Zero the entire page first (for BSS and partial pages)
                unsafe {
                    core::ptr::write_bytes(kernel_ptr, 0, 4096);
                }

                // Calculate what portion of the segment falls in this page
                let page_start = page_vaddr;
                let page_end = page_vaddr + 4096;

                // Calculate the range of the segment that overlaps with this page
                let seg_start = vaddr_start;
                let seg_file_end = vaddr_start + filesz; // End of file data

                // Only copy if this page contains file data
                if seg_file_end > page_start && seg_start < page_end {
                    // Calculate the overlap between segment file data and this page
                    let copy_start = seg_start.max(page_start);
                    let copy_end = seg_file_end.min(page_end);
                    let copy_len = (copy_end - copy_start) as usize;

                    if copy_len > 0 {
                        // Calculate source offset in ELF file
                        let file_offset = offset + (copy_start - vaddr_start);
                        let src = &data[file_offset as usize..(file_offset as usize + copy_len)];

                        // Calculate destination offset within the page
                        let page_offset = (copy_start - page_vaddr) as usize;
                        let dest = unsafe { kernel_ptr.add(page_offset) };

                        serial_println!(
                            "      Copying {} bytes at offset {} in page",
                            copy_len,
                            page_offset
                        );
                        unsafe {
                            core::ptr::copy_nonoverlapping(src.as_ptr(), dest, copy_len);
                        }
                    }
                }
            }
        }
    }

    // Allocate user stack pages
    let stack_bottom = USER_STACK_TOP - USER_STACK_SIZE;
    let stack_flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::USER_ACCESSIBLE
        | PageTableFlags::NO_EXECUTE;

    serial_println!(
        "  Allocating stack: 0x{:x} - 0x{:x} ({} pages)",
        stack_bottom,
        USER_STACK_TOP,
        USER_STACK_PAGES
    );

    for page_addr in (stack_bottom..USER_STACK_TOP).step_by(4096) {
        serial_println!("    Allocating stack page 0x{:x}", page_addr);

        // Map the stack page and get physical address
        let phys_addr = map_user_page(
            mapper,
            frame_allocator,
            VirtAddr::new(page_addr),
            stack_flags,
        )
        .map_err(|e| Error::MappingFailed(e))?;

        serial_println!("      Mapped to phys 0x{:x}", phys_addr.as_u64());

        // Zero the stack page through kernel's physical memory mapping
        let kernel_ptr = (phys_mem_offset.as_u64() + phys_addr.as_u64()) as *mut u8;
        serial_println!("      Zeroing via kernel ptr 0x{:x}", kernel_ptr as u64);
        unsafe {
            core::ptr::write_bytes(kernel_ptr, 0, 4096);
        }
        serial_println!("      Done");
    }

    serial_println!("  ELF loaded successfully, entry=0x{:x}", entry);

    Ok(ElfLoadResult {
        entry_point: entry,
        stack_top: USER_STACK_TOP,
    })
}
