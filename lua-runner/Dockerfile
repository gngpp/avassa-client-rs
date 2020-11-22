FROM alpine

RUN apk add libgcc
COPY alpine-target/release/lua-runner /usr/local/bin/.
CMD lua-runner
