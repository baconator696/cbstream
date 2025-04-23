FROM --platform=linux/amd64 rust AS build
ARG TARGETPLATFORM
WORKDIR /build/
COPY . /build/
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ] ;then\
    dpkg --add-architecture arm64 &&\
    apt update && apt install gcc-aarch64-linux-gnu libssl-dev:arm64 -y &&\
    cp -rs /usr/lib/aarch64-linux-gnu/* /usr/aarch64-linux-gnu/lib ;\
    rustup target add aarch64-unknown-linux-gnu &&\
    OPENSSL_DIR=/usr/aarch64-linux-gnu \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="aarch64-linux-gnu-gcc" \
    cargo build -r --target aarch64-unknown-linux-gnu &&\
    mkdir /target &&\
    mv /build/target/aarch64-unknown-linux-gnu/release/cbstream-rust /target/cbstream &&\
    ldd /target/cbstream | grep -o '/[^ ]*' | xargs -I {} cp --parents {} /target/ &&\
    elif [ "$TARGETPLATFORM" = "linux/amd64" ] ;then\
    apt update && apt install gcc libssl-dev pkg-config -y &&\
    cargo build -r &&\
    mkdir /target &&\
    mv /build/target/release/cbstream-rust /target/cbstream &&\
    ldd /target/cbstream | grep -o '/[^ ]*' | xargs -I {} cp --parents {} /target/ &&\
    else \
    echo "Unsupported platform: $TARGETPLATFORM"; exit 1;\
    fi
FROM scratch
WORKDIR /
COPY --from=build /target/lib64 /lib64
COPY --from=build /target/lib /lib
COPY --from=build /etc/ssl /etc/ssl
COPY --from=build /target/cbstream /bin/cbstream
ENTRYPOINT ["cbstream"]