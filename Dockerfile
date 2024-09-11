FROM rust:1.81

WORKDIR /usr/src/webcom_ical
COPY ./src ./src
COPY Cargo.lock ./
COPY Cargo.toml ./

RUN cargo install --path .

CMD ["webcom_ical"]