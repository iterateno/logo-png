###########
# BACKEND #
###########

FROM rust:1.37.0-slim-stretch AS build_backend

RUN apt-get -y update && apt-get -y install pkg-config libssl-dev libpq-dev

RUN mkdir -p /build/src

WORKDIR /build

# Backend Dependencies

RUN echo "fn main() {println!(\"Empty\")}" > src/main.rs

COPY Cargo.lock .
COPY Cargo.toml .

RUN cargo build --release


# Backend build

COPY src src
# Force rebuild
RUN touch src/main.rs
RUN cargo build --release

############
# FRONTEND #
############

FROM node:18 AS build_history_frontend
ENV NODE_ENV production

RUN yarn global add elm

RUN mkdir /app
WORKDIR /app

COPY history-frontend/elm.json .
COPY history-frontend/src src

# Download dependencies
RUN mkdir build/ && \
    elm make src/Main.elm --optimize --output=build/history.js

COPY history-frontend/history.html build/


###########
# RUNTIME #
###########

FROM debian:stretch-slim AS release

RUN apt-get -y update && apt-get -y install pkg-config libssl1.1 libpq5 ca-certificates wget && rm -rf /var/lib/apt/lists/*

RUN wget -O /usr/local/bin/dumb-init https://github.com/Yelp/dumb-init/releases/download/v1.2.2/dumb-init_1.2.2_amd64
RUN chmod +x /usr/local/bin/dumb-init

WORKDIR /app

RUN mkdir history-frontend/
COPY --from=build_history_frontend /app/build/* history-frontend/


RUN mkdir /app/src
COPY --from=build_backend /build/target/release/logo-png .
COPY --from=build_backend /build/src/*.html src/


EXPOSE 3000

RUN groupadd -g 999 -r logo-png && useradd -r -u 999 -g logo-png logo-png
USER logo-png

ENTRYPOINT ["/usr/local/bin/dumb-init", "--"]
CMD ["/app/logo-png"]
