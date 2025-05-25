FROM --platform=$BUILDPLATFORM rust AS build
ARG TARGETPLATFORM
ARG BUILDPLATFORM
WORKDIR /build/
COPY . /build/
RUN bash docker-build.sh $TARGETPLATFORM $BUILDPLATFORM
FROM scratch
LABEL org.opencontainers.image.source=https://github.com/baconator696/cbstream
WORKDIR /
COPY --from=build /target/root /
COPY --from=build /etc/ssl /etc/ssl
COPY --from=build /target/cbstream /bin/cbstream
COPY --from=build /target/ffmpeg /bin/ffmpeg
ENTRYPOINT ["cbstream"]