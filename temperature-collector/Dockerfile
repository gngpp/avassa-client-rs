FROM alpine

RUN apk add libgcc

COPY target/release/temperature-collector /usr/local/bin/.
COPY target/release/temperature-aggregator /usr/local/bin/.
