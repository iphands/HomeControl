FROM debian:trixie-slim
WORKDIR /src
RUN apt update && apt install -y cargo
COPY ./app /src/app
COPY ./app_rs /src/app_rs
RUN cd /src/app_rs && cargo build --release
CMD [ "/src/app_rs/target/release/homectl" ]
