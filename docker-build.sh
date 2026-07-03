#!/bin/bash
set -euo pipefail
trap 'echo "Error on line $LINENO: $BASH_COMMAND" >&2' ERR

pull_ffmpeg_latest_tag() {
    local tag=$(git ls-remote --tags https://github.com/FFmpeg/FFmpeg | awk -F'/' '{print $3}' | grep -v '\^{}' | grep '^n[0-9]' | grep -v -e '\-rc' -e 'dev' | sort -V | tail -1)
    git clone --depth 1 --branch $tag https://github.com/FFmpeg/FFmpeg
}
build_ffmpeg() {
    local build=$WD/build
    PREFIX=$build/prefix
    mkdir -p $PREFIX
    cd $build
    ../FFmpeg/configure \
        --prefix=$PREFIX \
        --cc=clang --cxx=clang++ --ld=clang --host-cc=clang --host-ld=clang \
        --ar=llvm-ar --nm=llvm-nm --ranlib=llvm-ranlib --strip=llvm-strip \
        --enable-cross-compile --arch=$ARCH --target-os=linux \
        --extra-cflags=" \
            --target=${ARCH}-linux-musl \
        " \
        --extra-ldflags=" \
            --target=${ARCH}-linux-musl \
            -fuse-ld=lld \
            -static \
        " \
        --enable-static --disable-shared --disable-debug \
        --disable-everything --disable-ffprobe --enable-ffmpeg --enable-protocol=file \
        --disable-swscale --disable-swresample --disable-avdevice \
        --enable-demuxer=mov --enable-demuxer=mpegts --enable-muxer=matroska \
        --enable-parser=aac --enable-parser=h264 \
        --enable-decoder=aac --enable-bsf=extract_extradata
    make install -j$(nproc)
    cd $WD
}
WD=$(pwd)
# Installs all dependencies and compiles
if [ "$1" = "linux/arm64" ]; then
    ARCH=aarch64
    ARCH_CAP=AARCH64
    ARCH_ALT=arm64
elif [ "$1" = "linux/amd64" ]; then
    ARCH=x86_64
    ARCH_CAP=X86_64
    ARCH_ALT=amd64
else
    echo "Unsupported target platform: $1"
    exit 1
fi
dpkg --add-architecture $ARCH_ALT
apt update
apt install -y git llvm clang lld make pkg-config nasm musl-dev:$ARCH_ALT
rustup target add ${ARCH}-unknown-linux-musl
env CC_${ARCH}_unknown_linux_musl=clang \
    CARGO_TARGET_${ARCH_CAP}_UNKNOWN_LINUX_MUSL_LINKER=clang \
    CFLAGS_${ARCH}_unknown_linux_musl=-I/usr/include/${ARCH}-linux-musl \
    RUSTFLAGS=" \
            -C link-arg=-fuse-ld=lld \
            -C link-arg=--target=${ARCH}-linux-musl \
            -C link-arg=-static \
    " \
    cargo build -r --target ${ARCH}-unknown-linux-musl
# Build FFmpeg
pull_ffmpeg_latest_tag
build_ffmpeg
# Copies all needed binaries/libraries for export
mkdir -p /target/root/bin                     # holding dir for excs
mv $PREFIX/bin/ffmpeg /target/root/bin/ffmpeg # move ffmpeg to holding dir
mv $WD/target/${ARCH}-unknown-linux-musl/release/cbstream-rust /target/root/bin/cbstream
