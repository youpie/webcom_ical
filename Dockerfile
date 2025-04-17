FROM rust:slim

WORKDIR /usr/src/webcom_ical
COPY ./src ./src
COPY Cargo.lock ./
COPY Cargo.toml ./
COPY ./templates templates
COPY ./kuma kuma

RUN cargo install --path .

CMD ["webcom_ical"]
