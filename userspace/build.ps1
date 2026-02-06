# Build script for userspace programs
# Requires: nasm, ld (from binutils or llvm)

$ErrorActionPreference = "Stop"

Write-Host "Setting current directory to script location" -ForegroundColor Cyan
Set-Location -Path $PSScriptRoot

Write-Host "Building hello.asm -> hello.elf" -ForegroundColor Cyan

# Assemble
nasm -f elf64 -o hello.o hello.asm
if ($LASTEXITCODE -ne 0) { throw "nasm failed" }

# Link with our linker script
# Use lld (LLVM linker) if available, otherwise ld
$linker = "ld.lld"
try {
    & $linker --version 2>$null | Out-Null
} catch {
    $linker = "ld"
}

& $linker -nostdlib -static -T linker.ld -o hello.elf hello.o
if ($LASTEXITCODE -ne 0) { throw "linker failed" }

Write-Host "Built hello.elf successfully!" -ForegroundColor Green

# Copy to kernel resources
Copy-Item hello.elf ..\kernel\src\resources\hello.elf -Force -ErrorAction SilentlyContinue
Write-Host "`nCopied to kernel/src/resources/hello.elf" -ForegroundColor Green
