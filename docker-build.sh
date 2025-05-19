# arm64 build
if [ "$1" = "linux/arm64" ]; then
    # installs dependencies
    dpkg --add-architecture arm64 &&
        apt update && apt install gcc-aarch64-linux-gnu libssl-dev:arm64 -y &&
        cp -rs /usr/lib/aarch64-linux-gnu/* /usr/aarch64-linux-gnu/lib
    # install rust and compile
    rustup target add aarch64-unknown-linux-gnu &&
        OPENSSL_DIR=/usr/aarch64-linux-gnu \
            CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="aarch64-linux-gnu-gcc" \
            cargo build -r --target aarch64-unknown-linux-gnu &&
        mkdir /target &&
        mv /build/target/aarch64-unknown-linux-gnu/release/cbstream-rust /target/cbstream &&
        mkdir /target/root &&
        cp --parents /lib/aarch64-linux-gnu/libssl.so.3 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libcrypto.so.3 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libgcc_s.so.1 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libc.so.6 /target/root/ &&
        cp --parents /lib/ld-linux-aarch64.so.1 /target/root/
# amd64 build
elif [ "$1" = "linux/amd64" ]; then
    # isntall dependencies and compile
    apt update && apt install gcc libssl-dev pkg-config -y &&
        cargo build -r &&
        mkdir /target &&
        mv /build/target/release/cbstream-rust /target/cbstream &&
        mkdir /target/root &&
        ldd /target/cbstream | grep -o '/[^ ]*' | xargs -I {} cp --parents {} /target/root/
else
    echo "Unsupported platform: $1"
    exit 1
fi
