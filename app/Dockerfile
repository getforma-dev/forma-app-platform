FROM node:22.21.0-bookworm-slim@sha256:f9f7f95dcf1f007b007c4dcd44ea8f7773f931b71dc79d57c216e731c87a090b AS browser
WORKDIR /source/admin
RUN npm install --global npm@11.6.2
COPY app/admin/package.json app/admin/package-lock.json ./
RUN npm ci
COPY app/admin/build.ts app/admin/tsconfig.json ./
COPY app/admin/src ./src
RUN npm run typecheck && npm run build

FROM rust:1.93.1-bookworm@sha256:7c4ae649a84014c467d79319bbf17ce2632ae8b8be123ac2fb2ea5be46823f31 AS native
WORKDIR /source/app
ENV CARGO_BUILD_JOBS=2
ARG TARGETARCH
COPY app/Cargo.toml app/Cargo.lock ./
COPY app/src ./src
COPY app/migrations ./migrations
COPY spikes/APP-006/Cargo.toml /source/spikes/APP-006/Cargo.toml
COPY spikes/APP-006/src /source/spikes/APP-006/src
# Compile-time inputs embedded by non-test code (include_str!): the composer's
# two few-shot fixtures (composer/prompt.rs) and the library router sidecar
# (storage/queries.rs). Each is listed in .dockerignore; app/qualification/
# test_tools.py proves this set covers every such include.
COPY contracts/fixtures/impl-2026-10-28.1/northstar.json contracts/fixtures/impl-2026-10-28.1/spend-review.json /source/contracts/fixtures/impl-2026-10-28.1/
COPY contracts/fixtures/library/catalog.json /source/contracts/fixtures/library/catalog.json
COPY --from=browser /source/admin/dist ./admin/dist
RUN --mount=type=cache,id=genui-cargo-registry,target=/usr/local/cargo/registry,sharing=locked --mount=type=cache,id=genui-cargo-target-${TARGETARCH},target=/source/app/target cargo build --release --locked && cp /source/app/target/release/forma-app-platform /source/genui

# parser-host and its WASM guest: their own workspace, lockfile and pinned
# toolchain, never in the app's Cargo graph (ADR-009 D9.1). Paths are fixed so
# parser/.cargo/config.toml's --remap-path-prefix keeps them out of the guest.
# One build job here plus two in `native` keeps aggregate Cargo jobs at three.
# The two stages run in parallel on one registry cache, and Cargo's
# package-cache lock lives outside that mount, so both mount it sharing=locked.
FROM rust:1.93.1-bookworm@sha256:7c4ae649a84014c467d79319bbf17ce2632ae8b8be123ac2fb2ea5be46823f31 AS parser
WORKDIR /source/parser
ENV CARGO_BUILD_JOBS=1
ARG TARGETARCH
RUN rustup target add --toolchain 1.93.1 wasm32-unknown-unknown && rustup component add --toolchain 1.93.1 clippy rustfmt
COPY parser/ ./
RUN --mount=type=cache,id=genui-cargo-registry,target=/usr/local/cargo/registry,sharing=locked --mount=type=cache,id=genui-parser-target-${TARGETARCH},target=/source/parser/target \
    python3 verify_vendor.py \
    && cargo build --locked --release -p parser-guest --target wasm32-unknown-unknown \
    && cargo build --locked --release -p parser-host \
    && mkdir /source/parser-out \
    && cp target/wasm32-unknown-unknown/release/parser_guest.wasm target/release/parser-host /source/parser-out/ \
    && rustc -vV > /source/rustc-vv.txt \
    && python3 write_manifest.py --source /source/parser \
        --guest /source/parser-out/parser_guest.wasm --guest-path /app/parser/parser_guest.wasm \
        --host /source/parser-out/parser-host --host-path /app/parser/parser-host \
        --rustc-vv /source/rustc-vv.txt --output /source/parser-out/parser-manifest.json

FROM python:3.11.14-slim-bookworm@sha256:65a93d69fa75478d554f4ad27c85c1e69fa184956261b4301ebaf6dbb0a3543d
# Debian security fixes for the base image's openssl, gnutls and perl (APP-078 step 3.5, ECR scan of
# 2026-10-30: 10 CRITICAL in these packages). Exact versions from bookworm-security; bump them with the base digest.
RUN apt-get update \
    && apt-get install -y --no-install-recommends --only-upgrade \
       libssl3=3.0.22-1~deb12u2 openssl=3.0.22-1~deb12u2 libgnutls30=3.7.9-2+deb12u7 perl-base=5.36.0-7+deb12u4 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY app/runtime/requirements.lock ./requirements.lock
RUN python -m pip install --no-cache-dir --require-hashes --only-binary=:all: -r requirements.lock
COPY --from=parser /source/parser-out/parser_guest.wasm /source/parser-out/parser-host /app/parser/
COPY --from=parser /source/parser-out/parser-manifest.json /app/parser-manifest.json
COPY --from=native /source/genui /app/genui
COPY app/runtime/runtime.py /app/runtime.py
# APP-012 (ADR-007 D7.10): the confined DuckDB engine child, `runtime.py query`.
COPY app/runtime/query_engine.py /app/query_engine.py
COPY app/migrations /app/migrations
COPY app/certs/rds-global-bundle.pem /etc/ssl/certs/rds-global-bundle.pem
RUN mkdir -p /run/genui && chown 65532:65532 /run/genui && chmod 0700 /run/genui
VOLUME ["/run/genui"]
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 GENUI_RUNTIME_CONFIG_FILE=/run/genui/credentials.json GENUI_RDS_CA_FILE=/etc/ssl/certs/rds-global-bundle.pem AWS_CONFIG_FILE=/dev/null AWS_SHARED_CREDENTIALS_FILE=/dev/null GENUI_PARSER_MANIFEST_FILE=/app/parser-manifest.json
USER 65532:65532
EXPOSE 3000
ENTRYPOINT ["python3", "/app/runtime.py"]
CMD ["serve"]
