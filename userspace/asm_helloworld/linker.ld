/* Linker script for lymadOS userspace programs */

ENTRY(_start)

SECTIONS
{
    /* Load at conventional user address */
    . = 0x400000;

    .text : {
        *(.text)
        *(.text.*)
    }

    . = ALIGN(4096);

    .rodata : {
        *(.rodata)
        *(.rodata.*)
    }

    . = ALIGN(4096);

    .data : {
        *(.data)
        *(.data.*)
    }

    . = ALIGN(4096);

    .bss : {
        *(.bss)
        *(.bss.*)
        *(COMMON)
    }

    /DISCARD/ : {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
    }
}
