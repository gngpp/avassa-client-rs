FROM alpine

RUN apk add libgcc
COPY target/release/lua-runner /usr/local/bin/.
CMD lua-runner
