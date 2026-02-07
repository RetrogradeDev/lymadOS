# Just because I'm too stupid to remember this
cargo build --release --target x86_64-unknown-none

# Copy to kernel resources
Copy-Item target\x86_64-unknown-none\release\hello_world ..\..\kernel\src\resources\hello_world.elf -Force