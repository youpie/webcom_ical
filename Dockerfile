FROM rust:1.89.0 AS builder

WORKDIR /usr/src/webcom_ical
COPY ./src ./src
COPY Cargo.lock ./
COPY Cargo.toml ./
COPY ./kuma ./kuma

RUN cargo install --path .

# Final image
FROM debian:bookworm-slim

ARG firefox_ver=130.0
ARG geckodriver_ver=0.35.0

RUN apt-get update \
    && apt-get upgrade -y \
    && apt-get install -y --no-install-recommends --no-install-suggests \
    ca-certificates curl bzip2 libgl1 libpci3 \
    `apt-cache depends firefox-esr | awk '/Depends:/{print$2}'` \
    && update-ca-certificates \
    \
    # Install Firefox
    && curl -fL -o /tmp/firefox.tar.bz2 \
    https://ftp.mozilla.org/pub/firefox/releases/${firefox_ver}/linux-x86_64/en-GB/firefox-${firefox_ver}.tar.bz2 \
    && tar -xjf /tmp/firefox.tar.bz2 -C /opt/ \
    && mv /opt/firefox /opt/firefox-${firefox_ver} \
    && ln -s /opt/firefox-${firefox_ver} /opt/firefox \
    \
    # Install geckodriver
    && curl -fL -o /tmp/geckodriver.tar.gz \
    https://github.com/mozilla/geckodriver/releases/download/v${geckodriver_ver}/geckodriver-v${geckodriver_ver}-linux64.tar.gz \
    && tar -xzf /tmp/geckodriver.tar.gz -C /usr/local/bin/ \
    && chmod +x /usr/local/bin/geckodriver \
    \
    # Cleanup
    && rm -rf /var/lib/apt/lists/* /tmp/*

ENV MOZ_HEADLESS=1
ENV PATH="/usr/local/cargo/bin:${PATH}"

# Copy Rust binary from builder
COPY --from=builder /usr/local/cargo/bin/webcom_ical /usr/local/bin/
COPY ./templates /usr/src/webcom_ical/templates

# Start supervisor script that launches geckodriver + your app
COPY scripts/entrypoint.sh /usr/src/webcom_ical/entrypoint.sh
RUN chmod +x /usr/src/webcom_ical/entrypoint.sh

ENTRYPOINT ["/usr/src/webcom_ical/entrypoint.sh"]