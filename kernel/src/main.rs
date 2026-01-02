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
    let _mapper = unsafe { kernel::mm::memory::init(phys_mem_offset) };
    let frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    let mut buddy = kernel::mm::buddy::BuddyAllocator::new();
    serial_println!("Buddy Allocator initialized: {:p}", &buddy as *const _);

    // Just grab all frames and add them to the buddy system for testing
    let mut frame_iter = frame_allocator.usable_frames();
    for frame in frame_iter.by_ref() {
        let phys_addr = frame.start_address();
        let virt_addr = phys_mem_offset + phys_addr.as_u64();
        unsafe { buddy.add_frame(virt_addr.as_mut_ptr()) };
    }

    allocator::init_heap(buddy, phys_mem_offset).expect("Heap initialization failed");

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

    kernel::drivers::exit::exit_qemu(kernel::drivers::exit::QemuExitCode::Success);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);

    kernel::drivers::exit::exit_qemu(kernel::drivers::exit::QemuExitCode::Failed);
}
