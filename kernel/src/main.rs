#![no_std]
#![no_main]

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};

#[cfg(not(test))]
use core::panic::PanicInfo;

use bootloader_api::{BootInfo, BootloaderConfig, config::Mapping, entry_point};

use kernel::{
    mm::{allocator, memory::BootInfoFrameAllocator, user::BuddyFrameAllocator},
    serial_println,
    tasks::{SCHEDULER, switch::switch_to_first_task, task::Task},
};
use x86_64::VirtAddr;
use x86_64::instructions::interrupts;

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
    allocator::init_heap(phys_mem_offset.as_u64() as usize);

    // Just grab all frames and add them to the buddy system for testing
    let mut frame_iter = frame_allocator.usable_frames();
    for frame in frame_iter.by_ref() {
        let phys_addr = frame.start_address();
        let virt_addr = phys_mem_offset + phys_addr.as_u64();

        unsafe { allocator::add_frame(virt_addr.as_mut_ptr()) };
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

    // Create user tasks
    serial_println!("Creating user tasks...");

    // Embed the hello.elf binary at compile time
    static HELLO_ELF: &[u8] = include_bytes!("resources/hello_world.elf");
    serial_println!("Embedded hello.elf: {} bytes", HELLO_ELF.len());

    // Use the buddy allocator for ELF loading
    let mut buddy_frame_alloc = BuddyFrameAllocator;

    serial_println!("About to load ELF...");

    let elf_task = match unsafe {
        Task::from_elf(
            HELLO_ELF,
            &mut mapper,
            &mut buddy_frame_alloc,
            phys_mem_offset,
        )
    } {
        Ok(task) => task,
        Err(e) => {
            serial_println!("Failed to load ELF: {:?}", e);
            panic!("ELF loading failed");
        }
    };

    serial_println!(
        "ELF Task {} created (entry=0x{:x})",
        elf_task.id,
        elf_task.context.rip
    );

    {
        let mut scheduler = SCHEDULER.lock();

        scheduler.add_task(elf_task);

        serial_println!("Total tasks: {}", scheduler.task_count());

        // Start the scheduler
        scheduler.start();
    }

    serial_println!("Switching to first task...");

    // Switch to the first task (never returns)
    unsafe {
        switch_to_first_task();
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);

    kernel::drivers::exit::exit_qemu(kernel::drivers::exit::QemuExitCode::Failed);
}
