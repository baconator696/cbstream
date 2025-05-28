FROM --platform=$BUILDPLATFORM rust AS build
ARG TARGETPLATFORM
ARG BUILDPLATFORM
ARG TAG
ENV TAG="$TAG"
WORKDIR /build/
COPY . /build/
RUN bash docker-build.sh $TARGETPLATFORM $BUILDPLATFORM
FROM scratch
LABEL org.opencontainers.image.source=https://github.com/baconator696/cbstream
COPY --from=build /target/root /
COPY --from=build /etc/ssl /etc/ssl
WORKDIR /cbstream
ENTRYPOINT ["cbstream"]
