use bootloader_api::info::FrameBuffer;

use crate::mm::memory::BootInfoFrameAllocator;

pub struct Framebuffer {
    front_buffer: *mut u32, // the actual framebuffer
    back_buffer: *mut u32,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
}

impl Framebuffer {
    pub fn new(
        mut fb: FrameBuffer,
        allocator: &mut BootInfoFrameAllocator,
        phys_mem_offset: u64,
    ) -> Self {
        let info = fb.info();
        let front_buffer = fb.buffer_mut().as_mut_ptr() as *mut u32;
        let width = info.width;
        let height = info.height;
        let stride = info.stride;

        // Calculate pages needed
        let buffer_size = stride * height * 4;
        let pages_needed = (buffer_size + 4095) / 4096;

        // Allocate pages directly from buddy allocator
        let phys_addr = allocator
            .allocate_contiguous(pages_needed)
            .expect("Failed to allocate back buffer pages");
        let virt_addr = phys_addr.start_address() + phys_mem_offset;
        let back_buffer = virt_addr.as_u64() as *mut u32;

        // Zero the buffer
        unsafe {
            core::ptr::write_bytes(back_buffer, 0, stride * height);
        }

        Self {
            front_buffer,
            back_buffer,
            width,
            height,
            stride,
        }
    }

    pub fn flip(&mut self) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.back_buffer,
                self.front_buffer,
                self.stride * self.height,
            );
        }
    }

    pub fn get_back_buffer_ptr(&self) -> *mut u32 {
        self.back_buffer
    }
}
