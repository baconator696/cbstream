FROM --platform=linux/amd64 rust AS build
ARG TARGETPLATFORM
WORKDIR /build/
COPY . /build/
RUN ./docker-build.sh $TARGETPLATFORM
FROM scratch
LABEL org.opencontainers.image.source https://github.com/baconator696/cbstream
WORKDIR /
COPY --from=build /target/root /
COPY --from=build /etc/ssl /etc/ssl
COPY --from=build /target/cbstream /bin/cbstream
ENTRYPOINT ["cbstream"]