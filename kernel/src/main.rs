#![no_std]
#![no_main]

extern crate alloc;

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};

#[cfg(not(test))]
use core::panic::PanicInfo;

use bootloader_api::{BootInfo, BootloaderConfig, config::Mapping, entry_point};

use kernel::{
    mm::{allocator, memory::BootInfoFrameAllocator},
    serial_println,
    tasks::{SCHEDULER, Task, switch::switch_to_first_task},
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

    // Create user tasks
    serial_println!("Creating user tasks...");

    // Get the user entry code bytes (we'll copy this to each task)
    let user_code_1: &[u8] = &USER_TASK_1_CODE;
    let user_code_2: &[u8] = &USER_TASK_2_CODE;
    let user_code_3: &[u8] = &USER_TASK_3_CODE;

    {
        let mut scheduler = SCHEDULER.lock();

        // Create 3 user tasks
        let task1 = unsafe { Task::new(user_code_1, &mut mapper) };
        serial_println!("  Task {} created", task1.id);
        scheduler.add_task(task1);

        let task2 = unsafe { Task::new(user_code_2, &mut mapper) };
        serial_println!("  Task {} created", task2.id);
        scheduler.add_task(task2);

        let task3 = unsafe { Task::new(user_code_3, &mut mapper) };
        serial_println!("  Task {} created", task3.id);
        scheduler.add_task(task3);

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

// User task code - simple infinite loops
// Task 1: just spins with pause
static USER_TASK_1_CODE: [u8; 4] = [
    0xF3, 0x90, // pause
    0xEB, 0xFC, // jmp -4 (back to pause)
];

// Task 2: same as task 1
static USER_TASK_2_CODE: [u8; 4] = [
    0xF3, 0x90, // pause
    0xEB, 0xFC, // jmp -4
];

// Task 3: Make a syscall, then loop
// syscall instruction is 0x0F 0x05
static USER_TASK_3_CODE: [u8; 6] = [
    0x0F, 0x05, // syscall
    0xF3, 0x90, // pause
    0xEB, 0xFC, // jmp -4
];

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);

    kernel::drivers::exit::exit_qemu(kernel::drivers::exit::QemuExitCode::Failed);
}
