apt update &&
    apt install -y gcc-mingw-w64 gcc gcc-aarch64-linux-gnu pkg-config &&
    rustup target add x86_64-pc-windows-gnu aarch64-unknown-linux-gnu &&
    cargo build -r --target x86_64-pc-windows-gnu &&
    cargo build -r --target aarch64-unknown-linux-gnu &&
    cargo build -r &&
    mv /mnt/target/x86_64-pc-windows-gnu/release/cbstream-rust.exe /mnt/cbstream-win-amd64.exe &&
    mv /mnt/target/aarch64-unknown-linux-gnu/release/cbstream-rust /mnt/cbstream-linux-arm64 &&
    mv /mnt/target/release/cbstream-rust /mnt/cbstream-linux-amd64