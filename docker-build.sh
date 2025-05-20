echo $2
if [ "$2" != "linux/amd64" ]; then
    echo "Unsupported platform: $2"
    exit 1
fi
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
    apt install wget -y &&
        wget -O /etc/apt/keyrings/gpg-pub-moritzbunkus.gpg https://mkvtoolnix.download/gpg-pub-moritzbunkus.gpg &&
        echo "deb [signed-by=/etc/apt/keyrings/gpg-pub-moritzbunkus.gpg] https://mkvtoolnix.download/debian/ bookworm main" >/etc/apt/sources.list.d/mkvtoolnix.download.list &&
        apt update && apt install mkvtoolnix:arm64 -y &&
        mv /bin/mkvmerge /target/mkvmerge &&
        cp --parents /lib/aarch64-linux-gnu/libboost_filesystem.so.1.74.0 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libFLAC.so.12 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libz.so.1 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libfmt.so.9 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libQt6Core.so.6 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libgmp.so.10 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libstdc++.so.6 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libdvdread.so.8 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libvorbis.so.0 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libogg.so.0 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libm.so.6 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libgcc_s.so.1 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libdl.so.2 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libicui18n.so.72 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libicuuc.so.72 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libglib-2.0.so.0 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libdouble-conversion.so.3 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libb2.so.1 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libpcre2-16.so.0 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libzstd.so.1 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libdl.so.2 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libicudata.so.72 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libpcre2-8.so.0 /target/root/ &&
        cp --parents /lib/aarch64-linux-gnu/libgomp.so.1 /target/root/
# amd64 build
elif [ "$1" = "linux/amd64" ]; then
    # install dependencies and compile
    apt update && apt install gcc libssl-dev pkg-config -y &&
        cargo build -r &&
        mkdir /target &&
        mv /build/target/release/cbstream-rust /target/cbstream &&
        mkdir /target/root &&
        ldd /target/cbstream | grep -o '/[^ ]*' | xargs -I {} cp --parents {} /target/root/
    # install mkvtoolnix
    apt install wget -y &&
        wget -O /etc/apt/keyrings/gpg-pub-moritzbunkus.gpg https://mkvtoolnix.download/gpg-pub-moritzbunkus.gpg &&
        echo "deb [signed-by=/etc/apt/keyrings/gpg-pub-moritzbunkus.gpg] https://mkvtoolnix.download/debian/ bookworm main" >/etc/apt/sources.list.d/mkvtoolnix.download.list &&
        apt update && apt install mkvtoolnix -y &&
        mv /bin/mkvmerge /target/mkvmerge &&
        ldd /target/mkvmerge | grep -o '/[^ ]*' | xargs -I {} cp --parents {} /target/root/
else
    echo "Unsupported platform: $1"
    exit 1
fi
