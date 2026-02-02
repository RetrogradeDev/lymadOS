use x86_64::{
    VirtAddr,
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
};

use spin::Lazy;

const STACK_SIZE: usize = 4096 * 5;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const PAGE_FAULT_IST_INDEX: u16 = 1;
pub const GENERAL_PROTECTION_FAULT_IST_INDEX: u16 = 2;

pub static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();

    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        // TODO: Use a proper frame allocator here
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

        let stack_start = VirtAddr::from_ptr(&raw const STACK);
        let stack_end = stack_start + STACK_SIZE as u64;
        stack_end
    };

    tss.interrupt_stack_table[PAGE_FAULT_IST_INDEX as usize] = {
        // TODO: Use a proper frame allocator here
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

        let stack_start = VirtAddr::from_ptr(&raw const STACK);
        let stack_end = stack_start + STACK_SIZE as u64;
        stack_end
    };

    tss.interrupt_stack_table[GENERAL_PROTECTION_FAULT_IST_INDEX as usize] = {
        // TODO: Use a proper frame allocator here
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

        let stack_start = VirtAddr::from_ptr(&raw const STACK);
        let stack_end = stack_start + STACK_SIZE as u64;
        stack_end
    };

    // RSP0: Stack to use when switching from Ring 3 to Ring 0
    // Point to the top of our statically allocated kernel stack
    tss.privilege_stack_table[0] = {
        static mut KERNEL_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE]; // TODO: Do something better here

        let stack_start = VirtAddr::from_ptr(&raw const KERNEL_STACK);
        let stack_end = stack_start + STACK_SIZE as u64;
        stack_end
    };

    tss
});

pub struct Selectors {
    pub code: SegmentSelector,
    pub data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub user_data: SegmentSelector,
    pub tss: SegmentSelector,
}

pub static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();

    let code = gdt.append(Descriptor::kernel_code_segment());
    let data = gdt.append(Descriptor::kernel_data_segment());

    let user_data = gdt.append(Descriptor::user_data_segment());
    let user_code = gdt.append(Descriptor::user_code_segment());

    let tss = gdt.append(Descriptor::tss_segment(&TSS));

    (
        gdt,
        Selectors {
            code,
            data,
            user_code,
            user_data,
            tss,
        },
    )
});

pub fn init() {
    use x86_64::instructions::segmentation::{CS, DS, SS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();

    unsafe {
        CS::set_reg(GDT.1.code);
        SS::set_reg(GDT.1.data);
        DS::set_reg(GDT.1.data);

        load_tss(GDT.1.tss);
    }
}
