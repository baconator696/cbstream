declare -A lib_hashmap
copy_lib() {
    local lib="$1"
    local target="$2"
    local gcc="$3"
    while read -r lib; do
        [[ -n "$lib" ]] || continue
        if [[ -z "${lib_hashmap["$lib"]}" ]]; then
            lib_hashmap["$lib"]=1
            local path=$("$gcc" -print-file-name="$lib" 2>/dev/null)
            [[ -n "$path" ]] || continue
            echo COPYING "$path" to "$target"
            cp --parents "$path" "$target"
            copy_lib "$path" "$target" "$gcc"
        fi
    done < <(readelf -d "$lib" | awk '/NEEDED/ { print $5 }' | tr -d '[]')
}
copy_lib_star() {
    local lib_dir="$1"
    local target="$2"
    local gcc="$3"
    find "$lib_dir" -type f | while IFS= read -r lib; do
        copy_lib "$lib" "$target" "$gcc"
    done
}
echo $2
if [ "$2" != "linux/amd64" ]; then
    echo "Unsupported platform: $2"
    exit 1
fi
# arm64 build
if [ "$1" = "linux/arm64" ]; then
    # installs dependencies
    dpkg --add-architecture arm64 &&
        apt update &&
        apt install gcc-aarch64-linux-gnu libssl-dev:arm64 ffmpeg:arm64 -y && # installs dependencies
        cp -rs /usr/lib/aarch64-linux-gnu/* /usr/aarch64-linux-gnu/lib
    rustup target add aarch64-unknown-linux-gnu &&
        OPENSSL_DIR=/usr/aarch64-linux-gnu \
            CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="aarch64-linux-gnu-gcc" \
            cargo build -r --target aarch64-unknown-linux-gnu && # install rust and compile
        mkdir /target &&
        mv /build/target/aarch64-unknown-linux-gnu/release/cbstream-rust /target/cbstream &&
        mkdir /target/root &&
        ln -s /usr/lib/aarch64-linux-gnu /target/root/lib &&
        ln -s /usr/lib/aarch64-linux-gnu /target/root/lib64 &&
        copy_lib /target/cbstream /target/root/ aarch64-linux-gnu-gcc &&
        mv /bin/ffmpeg /target/ffmpeg &&
        copy_lib /target/ffmpeg /target/root/ aarch64-linux-gnu-gcc &&
        cp -r --parents /usr/lib/aarch64-linux-gnu/pulseaudio /target/root/ &&
        copy_lib_star /usr/lib/aarch64-linux-gnu/pulseaudio /target/root/ aarch64-linux-gnu-gcc

# amd64 build
elif [ "$1" = "linux/amd64" ]; then
    apt update &&
        apt install -y gcc libssl-dev pkg-config ffmpeg && # install dependencies
        cargo build -r &&                                  # compile rust
        mkdir /target &&
        mv /build/target/release/cbstream-rust /target/cbstream &&
        mkdir /target/root &&
        ln -s /usr/lib/x86_64-linux-gnu /target/root/lib &&
        ln -s /usr/lib/x86_64-linux-gnu /target/root/lib64 &&
        copy_lib /target/cbstream /target/root/ gcc &&
        mv /bin/ffmpeg /target/ffmpeg &&
        copy_lib /target/ffmpeg /target/root/ gcc &&
        cp -r --parents /usr/lib/x86_64-linux-gnu/pulseaudio /target/root/ &&
        copy_lib_star /usr/lib/x86_64-linux-gnu/pulseaudio /target/root/ gcc
else
    echo "Unsupported platform: $1"
    exit 1
fi
