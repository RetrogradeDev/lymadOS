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

const BOOT_STACK_TOP: VirtAddr = VirtAddr::new((0x8e0000 + STACK_SIZE as u64) - 16);

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

    tss.privilege_stack_table[0] = BOOT_STACK_TOP;

    tss
});

pub struct Selectors {
    code: SegmentSelector,
    data: SegmentSelector,
    user_code: SegmentSelector,
    user_data: SegmentSelector,
    tss: SegmentSelector,
}

pub static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();

    let code = gdt.append(Descriptor::kernel_code_segment());
    let data = gdt.append(Descriptor::kernel_data_segment());
    let user_code = gdt.append(Descriptor::user_code_segment());
    let user_data = gdt.append(Descriptor::user_data_segment());
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
