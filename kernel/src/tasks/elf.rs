// Elf parser and loader

use goblin::elf::Elf;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, PageTableFlags, Size4KiB},
};

use crate::{mm::user::map_user_page, serial_println};

pub enum Error {
    MappingFailed(&'static str),
    InvalidElf(goblin::error::Error),
}

pub fn load_elf(
    data: &[u8],
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<u64, Error> {
    let file = Elf::parse(data).map_err(|e| Error::InvalidElf(e))?;

    for ph in &file.program_headers {
        if ph.p_type == goblin::elf::program_header::PT_LOAD {
            // Map pages at p_vaddr with size p_memsz
            // Copy p_filesz bytes from data[ph.p_offset..ph.p_offset + ph.p_filesz] to p_vaddr
            // Zero the remaining bytes up to p_memsz

            let vaddr_start = ph.p_vaddr;
            let memsz = ph.p_memsz;

            // Determinate page flags
            let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            if ph.p_flags & goblin::elf::program_header::PF_W != 0 {
                flags |= PageTableFlags::WRITABLE;
            }
            if ph.p_flags & goblin::elf::program_header::PF_X == 0 {
                flags |= PageTableFlags::NO_EXECUTE;
            }

            // Map all pages for this segment
            let start_page = vaddr_start & !0xFFF;
            let end_page = (vaddr_start + memsz + 0xFFF) & !0xFFF;

            for page_addr in (start_page..end_page).step_by(4096) {
                map_user_page(mapper, frame_allocator, VirtAddr::new(page_addr), flags)
                    .map_err(|e| Error::MappingFailed(e))?;
            }

            // Copy data from ELF file
            let src = &data[ph.p_offset as usize..(ph.p_offset + ph.p_filesz) as usize];
            let dest = vaddr_start as *mut u8;
            unsafe {
                core::ptr::copy_nonoverlapping(src.as_ptr(), dest, ph.p_filesz as usize);

                // Zero BSS (memsz > filesz)
                let bss_start = dest.add(ph.p_filesz as usize);
                let bss_size = (ph.p_memsz - ph.p_filesz) as usize;
                core::ptr::write_bytes(bss_start, 0, bss_size);
            }
        } else {
            serial_println!("Skipping non-loadable segment: type={}", ph.p_type);
        }
    }

    Ok(file.entry)
}
