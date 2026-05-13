# Build Guide

## Windows prerequisites

- Rust toolchain for `x86_64-pc-windows-msvc`
- Visual Studio 2022 Build Tools with `Desktop development with C++`
- CMake
- A working `cl.exe` / MSVC environment

If `cmake.exe` is not on `PATH`, set the `CMAKE` environment variable explicitly before building.

Typical Visual Studio Build Tools CMake path:

```powershell
$env:CMAKE='C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe'
```

## Build

From the repository root:

```powershell
cargo check
cargo test
```

If your environment has no network access and dependencies are already cached locally:

```powershell
cargo check --offline
cargo test --offline
```

## Notes

- The project depends on `whisper-rs-sys`, which builds `whisper.cpp` through CMake.
- On Windows, a local CMake path is enough; it does not need to be globally added to `PATH` if `CMAKE` is set.
- If build artifacts were generated with a broken toolchain setup, clean them first:

```powershell
cargo clean
```
