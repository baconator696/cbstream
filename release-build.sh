docker build -t rustcross -f Dockerfile.cross . &&
docker run --rm \
    -v $(pwd):/mnt -w /mnt \
    -e TAG="$TAG" \
    rustcross bash -c "
        dpkg --add-architecture arm64 &&
        apt update &&
        apt install -y llvm clang lld gcc-mingw-w64 pkg-config musl-dev musl-dev:arm64 &&
        rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-apple-darwin aarch64-apple-darwin &&
        CC_x86_64_unknown_linux_musl=clang \
            CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=clang \
            CFLAGS_x86_64_unknown_linux_musl='-I/usr/include/x86_64-linux-musl' \
            RUSTFLAGS='
                -C link-arg=-fuse-ld=lld
                -C link-arg=--target=x86_64-linux-musl
                -C link-arg=-static
            ' \
            cargo build -r --target x86_64-unknown-linux-musl &&
        CC_aarch64_unknown_linux_musl=clang \
            CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=clang \
            CFLAGS_aarch64_unknown_linux_musl='-I/usr/include/aarch64-linux-musl' \
            RUSTFLAGS='
                -C link-arg=-fuse-ld=lld
                -C link-arg=--target=aarch64-linux-musl
                -C link-arg=-static
            ' \
            cargo build -r --target aarch64-unknown-linux-musl &&
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
            RUSTFLAGS='-C target-feature=+crt-static' \
            cargo build -r --target x86_64-pc-windows-gnu &&
        CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER=o64h-clang \
            CC=o64h-clang cargo build -r --target x86_64-apple-darwin &&
        CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=oa64-clang \
            CC=oa64-clang cargo build -r --target aarch64-apple-darwin &&
        mv /mnt/target/x86_64-unknown-linux-musl/release/cbstream-rust /mnt/cbstream-linux-amd64 &&
        mv /mnt/target/aarch64-unknown-linux-musl/release/cbstream-rust /mnt/cbstream-linux-arm64 &&
        mv /mnt/target/x86_64-pc-windows-gnu/release/cbstream-rust.exe /mnt/cbstream-win-amd64.exe &&
        mv /mnt/target/x86_64-apple-darwin/release/cbstream-rust /mnt/cbstream-apple-amd64 &&
        mv /mnt/target/aarch64-apple-darwin/release/cbstream-rust /mnt/cbstream-apple-arm64
"
