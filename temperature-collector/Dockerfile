FROM alpine

RUN apk add libgcc

COPY alpine-target/release/temperature-collector /usr/local/bin/.
COPY alpine-target/release/temperature-aggregator /usr/local/bin/.
