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
            cp -L "$path" "$target"
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
# Installs all dependencies and compiles
export RUSTFLAGS="-C target-feature=+crt-static"
if [ "$1" = "linux/arm64" ]; then
    dpkg --add-architecture arm64 &&
        apt update &&
        apt install -y gcc gcc-aarch64-linux-gnu pkg-config ffmpeg:arm64 &&
        rustup target add aarch64-unknown-linux-gnu &&
        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
            cargo build -r --target aarch64-unknown-linux-gnu
elif [ "$1" = "linux/amd64" ]; then
    apt update &&
        apt install -y gcc pkg-config ffmpeg &&
        CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc \
            cargo build -r --target x86_64-unknown-linux-gnu
else
    echo "Unsupported platform: $1"
    exit 1
fi &&
    # Copies all needed binaries/libraries for export
    mkdir -p /target/root/bin &&
    mkdir -p /target/root/usr/local/lib/ &&
    ln -s /usr/local/lib /target/root/lib &&
    ln -s /usr/local/lib /target/root/lib64 &&
    cp /bin/ffmpeg /target/root/bin/ffmpeg &&
    if [ "$1" = "linux/arm64" ]; then
        mv /build/target/aarch64-unknown-linux-gnu/release/cbstream-rust /target/root/bin/cbstream &&
            copy_lib /bin/ffmpeg /target/root/usr/local/lib/ aarch64-linux-gnu-gcc &&
            cp -L /usr/lib/aarch64-linux-gnu/pulseaudio/libpulsecommon* /target/root/usr/local/lib/ &&
            copy_lib_star /usr/lib/aarch64-linux-gnu/pulseaudio/libpulsecommon* /target/root/usr/local/lib/ aarch64-linux-gnu-gcc
    elif [ "$1" = "linux/amd64" ]; then
        mv /build/target/x86_64-unknown-linux-gnu/release/cbstream-rust /target/root/bin/cbstream &&
            copy_lib /bin/ffmpeg /target/root/usr/local/lib/ gcc &&
            cp -L /usr/lib/x86_64-linux-gnu/pulseaudio/libpulsecommon* /target/root/usr/local/lib/ &&
            copy_lib_star /usr/lib/x86_64-linux-gnu/pulseaudio/libpulsecommon* /target/root/usr/local/lib/ gcc
    fi
