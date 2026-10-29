FROM rust:1.93.1-bookworm@sha256:7c4ae649a84014c467d79319bbf17ce2632ae8b8be123ac2fb2ea5be46823f31 AS native
WORKDIR /source
ENV CARGO_BUILD_JOBS=1
ARG TARGETARCH
COPY Cargo.toml Cargo.lock ./
COPY src/lib.rs ./src/lib.rs
COPY examples/hosted_key_qualification.rs ./examples/hosted_key_qualification.rs
RUN --mount=type=cache,id=genui-auth-cargo-registry,target=/usr/local/cargo/registry --mount=type=cache,id=genui-auth-qualifier-${TARGETARCH},target=/source/target cargo build --release --locked --example hosted_key_qualification && cp target/release/examples/hosted_key_qualification /source/qualify-hosted-keys

FROM python:3.11.14-slim-bookworm@sha256:65a93d69fa75478d554f4ad27c85c1e69fa184956261b4301ebaf6dbb0a3543d
COPY --from=native /source/qualify-hosted-keys /usr/local/bin/qualify-hosted-keys
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/qualify-hosted-keys"]
