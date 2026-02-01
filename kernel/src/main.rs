#![no_std]
#![no_main]

extern crate alloc;

#[cfg(not(test))]
use core::panic::PanicInfo;

use alloc::vec;
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use bootloader_api::{BootInfo, BootloaderConfig, config::Mapping, entry_point};

use kernel::{
    mm::{allocator, memory::BootInfoFrameAllocator},
    serial_println,
};
use x86_64::VirtAddr;
use x86_64::instructions::interrupts;
use x86_64::structures::paging::{Page, PageTableFlags, Size4KiB};

static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(main, config = &BOOTLOADER_CONFIG);

fn main(boot_info: &'static mut BootInfo) -> ! {
    kernel::init(); // If you dare to call serial_print before this, I'm not responsible for the consequences...

    serial_println!("Hello World!");

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());
    let mut mapper = unsafe { kernel::mm::memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    serial_println!("Initializing heap...");
    allocator::init_heap(phys_mem_offset.as_u64() as usize).expect("Heap initialization failed");

    // Just grab all frames and add them to the buddy system for testing
    let mut frame_iter = frame_allocator.usable_frames();
    for frame in frame_iter.by_ref() {
        let phys_addr = frame.start_address();
        let virt_addr = phys_mem_offset + phys_addr.as_u64();
        allocator::add_frame(virt_addr.as_mut_ptr());
    }

    // drop the iterator to make us able to borrow frame_allocator again later
    drop(frame_iter);

    // allocate a number on the heap
    let heap_value = Box::new(41);
    serial_println!("heap_value at {:p}", heap_value);

    // create a dynamically sized vector
    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    serial_println!("vec at {:p}", vec.as_slice());

    // create a reference counted vector -> will be freed when count reaches 0
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    serial_println!(
        "current reference count is {}",
        Rc::strong_count(&cloned_reference)
    );
    core::mem::drop(reference_counted);
    serial_println!(
        "reference count is {} now",
        Rc::strong_count(&cloned_reference)
    );

    serial_println!("Initializing APIC...");
    unsafe {
        kernel::drivers::apic::init(
            *boot_info.rsdp_addr.as_ref().unwrap() as usize,
            phys_mem_offset,
            &mut mapper,
            &mut frame_allocator,
        );
    };

    interrupts::enable();

    // Switch to user mode
    unsafe {
        use core::alloc::Layout;
        use core::ptr;

        // Allocate 1 page (4KiB) for user stack, aligned to 4KiB
        let stack_layout = Layout::from_size_align(4096, 4096).unwrap();
        let stack_ptr = alloc::alloc::alloc(stack_layout);
        if stack_ptr.is_null() {
            panic!("Failed to allocate stack for user task");
        }
        // Stack grows down, so top is end of allocation
        let stack_top = stack_ptr.add(stack_layout.size());

        // Allocate 1 page for user code
        let code_layout = Layout::from_size_align(4096, 4096).unwrap();
        let code_ptr = alloc::alloc::alloc(code_layout);
        if code_ptr.is_null() {
            panic!("Failed to allocate code page for user task");
        }

        // Copy the user_entry function to the user code page
        // We copy a fixed size that should be enough for our simple loop
        let user_fn_ptr = user_entry as *const u8;
        ptr::copy_nonoverlapping(user_fn_ptr, code_ptr, 64);

        // Mark the stack page as user-accessible (writable, not executable)
        // We need to set USER_ACCESSIBLE on all levels of the page table hierarchy
        let stack_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(stack_ptr as u64));
        set_page_user_accessible(&mut mapper, stack_page, true, false);

        // Mark the code page as user-accessible (executable, not writable)
        let code_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(code_ptr as u64));
        set_page_user_accessible(&mut mapper, code_page, false, true);

        serial_println!("Switching to user mode...");
        serial_println!("  User code at: {:p}", code_ptr);
        serial_println!("  User stack top at: {:p}", stack_top);

        enter_user_mode(code_ptr as usize, stack_top as usize);
    }

    // kernel::drivers::exit::exit_qemu(kernel::drivers::exit::QemuExitCode::Success);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);

    kernel::drivers::exit::exit_qemu(kernel::drivers::exit::QemuExitCode::Failed);
}

/// Set USER_ACCESSIBLE flag on all page table levels for a given page
/// This is needed because x86_64 requires the flag at ALL levels (PML4, PDPT, PD, PT)
/// Handles both 4KiB pages and 2MiB huge pages
unsafe fn set_page_user_accessible(
    mapper: &mut x86_64::structures::paging::OffsetPageTable,
    page: Page<Size4KiB>,
    writable: bool,
    executable: bool,
) {
    use x86_64::registers::control::Cr3;
    use x86_64::structures::paging::PageTable;

    let virt = page.start_address();
    let phys_offset = mapper.phys_offset();

    // Get the level 4 table
    let (l4_frame, _) = Cr3::read();
    let l4_table: &mut PageTable =
        unsafe { &mut *(phys_offset + l4_frame.start_address().as_u64()).as_mut_ptr() };

    // Level 4 entry
    let l4_entry = &mut l4_table[virt.p4_index()];
    l4_entry.set_flags(l4_entry.flags() | PageTableFlags::USER_ACCESSIBLE);

    // Level 3 table
    let l3_frame = l4_entry.frame().expect("L4 entry not present");
    let l3_table: &mut PageTable =
        unsafe { &mut *(phys_offset + l3_frame.start_address().as_u64()).as_mut_ptr() };
    let l3_entry = &mut l3_table[virt.p3_index()];
    l3_entry.set_flags(l3_entry.flags() | PageTableFlags::USER_ACCESSIBLE);

    // Level 2 table
    let l2_frame = l3_entry.frame().expect("L3 entry not present");
    let l2_table: &mut PageTable =
        unsafe { &mut *(phys_offset + l2_frame.start_address().as_u64()).as_mut_ptr() };
    let l2_entry = &mut l2_table[virt.p2_index()];

    // Check if this is a huge page (2MiB)
    if l2_entry.flags().contains(PageTableFlags::HUGE_PAGE) {
        // For huge pages, just set USER_ACCESSIBLE at L2 level
        let mut new_flags = l2_entry.flags() | PageTableFlags::USER_ACCESSIBLE;
        if writable {
            new_flags |= PageTableFlags::WRITABLE;
        }
        if executable {
            // Remove NO_EXECUTE flag if we want this page to be executable
            new_flags &= !PageTableFlags::NO_EXECUTE;
        }
        l2_entry.set_flags(new_flags);
    } else {
        // Normal 4KiB page - set flag at L2 and L1
        l2_entry.set_flags(l2_entry.flags() | PageTableFlags::USER_ACCESSIBLE);

        // Level 1 table (final page table entry)
        let l1_frame = l2_entry.frame().expect("L2 entry not present");
        let l1_table: &mut PageTable =
            unsafe { &mut *(phys_offset + l1_frame.start_address().as_u64()).as_mut_ptr() };
        let l1_entry = &mut l1_table[virt.p1_index()];

        let mut new_flags = l1_entry.flags() | PageTableFlags::USER_ACCESSIBLE;
        if writable {
            new_flags |= PageTableFlags::WRITABLE;
        }
        if executable {
            // Remove NO_EXECUTE flag if we want this page to be executable
            new_flags &= !PageTableFlags::NO_EXECUTE;
        }
        l1_entry.set_flags(new_flags);
    }

    // Flush the TLB for this page
    x86_64::instructions::tlb::flush(virt);
}

// NOTE: This function never returns
// It is the entry point for the user mode task
// Using naked function to ensure position-independent code
#[unsafe(naked)]
extern "C" fn user_entry() {
    // Simple infinite loop in assembly - completely position independent
    core::arch::naked_asm!(
        "2:", "pause",  // Hint to the CPU that we're spinning
        "jmp 2b", // Jump back to the label
    );
}

// Context switch to user mode
unsafe fn enter_user_mode(entry_point: usize, stack_pointer: usize) -> ! {
    use core::arch::asm;
    use kernel::gdt::GDT;
    use kernel::serial_println;
    use x86_64::registers::rflags::RFlags;

    // 1. Get User Selectors from GDT
    // Code Selector with RPL 3
    let user_code = (GDT.1.user_code.0 | 3) as u64;
    // Data Selector with RPL 3
    let user_data = (GDT.1.user_data.0 | 3) as u64;

    serial_println!("  User code selector: {:#x}", user_code);
    serial_println!("  User data selector: {:#x}", user_data);
    serial_println!("  Entry point: {:#x}", entry_point);
    serial_println!("  Stack pointer: {:#x}", stack_pointer);

    // 2. Enable Interrupts (IF bit) in RFLAGS so we can still handle timer/keyboard
    let rflags = RFlags::INTERRUPT_FLAG.bits();

    // 3. Prepare the stack frame for 'iretq'
    // IRETQ expects: SS, RSP, RFLAGS, CS, RIP
    // We also need to set DS/ES/FS/GS to user data segment

    unsafe {
        asm!(
            // Clear data segments to user data selector
            "mov ds, {ds:x}",
            "mov es, {ds:x}",
            "mov fs, {ds:x}",
            "mov gs, {ds:x}",

            // Push IRETQ frame
            "push {ss}",           // SS
            "push {rsp}",          // RSP
            "push {rflags}",       // RFLAGS
            "push {cs}",           // CS
            "push {rip}",          // RIP

            // Go!
            "iretq",

            ds = in(reg) user_data,
            ss = in(reg) user_data,
            rsp = in(reg) stack_pointer,
            rflags = in(reg) rflags,
            cs = in(reg) user_code,
            rip = in(reg) entry_point,
            options(noreturn)
        );
    }
}
