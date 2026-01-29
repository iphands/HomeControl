FROM debian:trixie-slim
WORKDIR /src
RUN apt update && apt install -y cargo
COPY ./frontend /app/frontend
COPY ./app_rs   /app/api
RUN cd /app/api && cargo build --release
CMD [ "/app/api/target/release/homectrl" ]
