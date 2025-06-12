docker build -t rustcross -f Dockerfile.cross . &&
    docker run --rm -it \
        -v $(pwd):/mnt -w /mnt \
        -e TAG="$TAG" \
        rustcross bash -c "
apt update &&
    apt install -y clang gcc gcc-mingw-w64 gcc-aarch64-linux-gnu pkg-config &&
    rustup target add x86_64-pc-windows-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin &&
    cargo build -r &&
    cargo build -r --target x86_64-pc-windows-gnu &&
    cargo build -r --target aarch64-unknown-linux-gnu &&
    CC=o64h-clang cargo build -r --target x86_64-apple-darwin &&
    CC=oa64-clang cargo build -r --target aarch64-apple-darwin &&
    mv /mnt/target/release/cbstream-rust /mnt/cbstream-linux-amd64 &&
    mv /mnt/target/x86_64-pc-windows-gnu/release/cbstream-rust.exe /mnt/cbstream-win-amd64.exe &&
    mv /mnt/target/aarch64-unknown-linux-gnu/release/cbstream-rust /mnt/cbstream-linux-arm64 &&
    mv /mnt/target/x86_64-apple-darwin/release/cbstream-rust /mnt/cbstream-apple-amd64 &&
    mv /mnt/target/aarch64-apple-darwin/release/cbstream-rust /mnt/cbstream-apple-arm64
"
