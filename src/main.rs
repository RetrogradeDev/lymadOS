use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};

#[cfg(test)]
mod allocator_tests; // I have no idea why rust shows an error but it works fine so idc

const UEFI_PATH: &str = env!("UEFI_PATH");

fn main() {
    println!("kernel binary at: {UEFI_PATH}");
    println!("Downloading OVMF firmware...");
    let prebuilt = Prebuilt::fetch(Source::LATEST, "target/omvf").expect("Failed to download OMVF");

    println!("Starting QEMU...");

    let code = prebuilt.get_file(Arch::X64, FileType::Code);
    let vars = prebuilt.get_file(Arch::X64, FileType::Vars);

    let mut cmd = std::process::Command::new("qemu-system-x86_64");

    cmd.arg("-m").arg("256M");
    cmd.arg("-serial").arg("stdio");

    // Disable graphics
    cmd.arg("-display").arg("none");

    // Enable debug exit
    cmd.arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");

    cmd.arg("-drive")
        .arg(format!("format=raw,file={}", UEFI_PATH));
    cmd.arg("-drive").arg(format!(
        "if=pflash,format=raw,unit=0,file={},readonly=on",
        code.display()
    ));
    // copy vars and enable rw instead of snapshot if you want to store data (e.g. enroll secure boot keys)
    cmd.arg("-drive").arg(format!(
        "if=pflash,format=raw,unit=1,file={},snapshot=on",
        vars.display()
    ));

    // Helps us when we reboot bc of a triple fault
    // cmd.arg("-d").arg("int");
    // cmd.arg("-no-reboot");

    let mut child = cmd.spawn().unwrap();
    let status = child.wait().unwrap();

    match status.code() {
        Some(code) => {
            if code == 0x11 {
                println!("QEMU exited with success.");
            } else {
                println!("QEMU exited with failure code: {code}");
            }
        }
        None => {
            println!("QEMU terminated by signal");
        }
    }
}
