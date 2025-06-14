docker build -t rustcross -f Dockerfile.cross . &&
docker run --rm \
    -v $(pwd):/mnt -w /mnt \
    -e TAG="$TAG" \
    rustcross bash -c "
apt update &&
    apt install -y clang gcc gcc-mingw-w64 gcc-aarch64-linux-gnu pkg-config &&
    rustup target add x86_64-pc-windows-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin &&
    export RUSTFLAGS='-C target-feature=+crt-static' &&
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc \
    cargo build -r --target x86_64-unknown-linux-gnu &&
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    cargo build -r --target aarch64-unknown-linux-gnu &&
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    cargo build -r --target x86_64-pc-windows-gnu &&
    CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER=o64h-clang \
    CC=o64h-clang cargo build -r --target x86_64-apple-darwin &&
    CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=oa64-clang \
    CC=oa64-clang cargo build -r --target aarch64-apple-darwin &&
    mv /mnt/target/x86_64-unknown-linux-gnu/release/cbstream-rust /mnt/cbstream-linux-amd64 &&
    mv /mnt/target/aarch64-unknown-linux-gnu/release/cbstream-rust /mnt/cbstream-linux-arm64 &&
    mv /mnt/target/x86_64-pc-windows-gnu/release/cbstream-rust.exe /mnt/cbstream-win-amd64.exe &&
    mv /mnt/target/x86_64-apple-darwin/release/cbstream-rust /mnt/cbstream-apple-amd64 &&
    mv /mnt/target/aarch64-apple-darwin/release/cbstream-rust /mnt/cbstream-apple-arm64
"
