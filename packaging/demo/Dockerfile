FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        dosfstools \
        e2fsprogs \
        fdisk \
        jq \
        mount \
        parted \
        util-linux \
        zstd \
    && rm -rf /var/lib/apt/lists/*

COPY rust-imager.deb /tmp/rust-imager.deb
RUN apt-get update \
    && apt-get install -y /tmp/rust-imager.deb \
    && rm -f /tmp/rust-imager.deb \
    && rm -rf /var/lib/apt/lists/*

COPY entrypoint.sh full-run.sh /usr/local/lib/rust-imager-demo/
RUN chmod 0755 /usr/local/lib/rust-imager-demo/*.sh

ENTRYPOINT ["/usr/local/lib/rust-imager-demo/entrypoint.sh"]
