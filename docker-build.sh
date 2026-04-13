declare -A lib_hashmap
# Recursively copies shared library dependencies from a source library to a target directory,
# using gcc's -print-file-name to resolve library paths and storing in a hash map
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
# Recursively copies all shared library dependencies from a directory tree to a target directory,
# processing each library file and its dependencies using the copy_lib function
copy_lib_star() {
    local lib_dir="$1"
    local target="$2"
    local gcc="$3"
    find "$lib_dir" -type f | while IFS= read -r lib; do
        copy_lib "$lib" "$target" "$gcc"
    done
}
if [ "$2" != "linux/amd64" ]; then
    echo "Unsupported build platform: $2"
    exit 1
fi
# Installs all dependencies and compiles
if [ "$1" = "linux/arm64" ]; then
    ## ARM64 BUILD
    dpkg --add-architecture arm64 &&
    apt update &&
    apt install -y gcc gcc-aarch64-linux-gnu pkg-config ffmpeg:arm64 &&
    rustup target add aarch64-unknown-linux-gnu &&
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
        cargo build -r --target aarch64-unknown-linux-gnu
elif [ "$1" = "linux/amd64" ]; then
    ## AMD64 BUILD
    apt update &&
    apt install -y gcc pkg-config ffmpeg &&
    CFLAGS='-march=x86-64 -mtune=generic' \
        CXXFLAGS='-march=x86-64 -mtune=generic' \
        RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx,-avx2,-fma' \
        CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc \
        cargo build -r --target x86_64-unknown-linux-gnu
else
    echo "Unsupported target platform: $1"
    exit 1
fi &&
    # Copies all needed binaries/libraries for export
    mkdir -p /target/root/bin &&                # holding dir for exes
    mkdir -p /target/root/usr/local/lib/ &&     # holding dir for libs
    ln -s /usr/local/lib /target/root/lib &&    # |
    ln -s /usr/local/lib /target/root/lib64 &&  # |-create symbolic link in holding dir
    cp /bin/ffmpeg /target/root/bin/ffmpeg &&   # move ffmpeg to holding dir
    if [ "$1" = "linux/arm64" ]; then
        ## ARM64
        mv /build/target/aarch64-unknown-linux-gnu/release/cbstream-rust /target/root/bin/cbstream &&   # move main exe to holding dir
            copy_lib /bin/ffmpeg /target/root/usr/local/lib/ aarch64-linux-gnu-gcc &&                   # use copy_lib copy ffmpeg libs to /target/root/usr/local/lib/
            cp -L /usr/lib/aarch64-linux-gnu/pulseaudio/libpulsecommon* /target/root/usr/local/lib/ &&  # CUSTOM may need change in future
            copy_lib_star /usr/lib/aarch64-linux-gnu/pulseaudio/libpulsecommon* /target/root/usr/local/lib/ aarch64-linux-gnu-gcc # CUSTOM copy all pulseaudio libs
    elif [ "$1" = "linux/amd64" ]; then
        # AMD64
        mv /build/target/x86_64-unknown-linux-gnu/release/cbstream-rust /target/root/bin/cbstream &&    # move main exe to holding dir
            copy_lib /bin/ffmpeg /target/root/usr/local/lib/ gcc &&                                     # use copy_lib copy ffmpeg libs to /target/root/usr/local/lib/
            cp -L /usr/lib/x86_64-linux-gnu/pulseaudio/libpulsecommon* /target/root/usr/local/lib/ &&   # CUSTOM may need change in future
            copy_lib_star /usr/lib/x86_64-linux-gnu/pulseaudio/libpulsecommon* /target/root/usr/local/lib/ gcc  # CUSTOM copy all pulseaudio libs
    fi
