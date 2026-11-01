# APP-075 isolated qualification image. Repository-root context restricted by
# app/qualification/Dockerfile.dockerignore (BuildKit's per-Dockerfile ignore
# file). Never the service image and never behind the load balancer: it holds
# the cfg(test) library test executable, the private supervisor and the
# reviewed RDS CA at its fixed path. Its entry point takes no paths,
# endpoints, roles or SQL.

# Export-only stage: `--target inputs --output type=local,dest=DIR` writes
# the exact build inputs for review; the final image never contains it.
FROM scratch AS inputs
COPY . /

FROM rust:1.93.1-bookworm@sha256:7c4ae649a84014c467d79319bbf17ce2632ae8b8be123ac2fb2ea5be46823f31 AS native
WORKDIR /source/app
ENV CARGO_BUILD_JOBS=2
ARG TARGETARCH
# ADR-013 D13.3: the root workspace manifest and lockfile at /source, the
# members at their repository paths; cargo finds the root walking up.
COPY Cargo.toml Cargo.lock /source/
COPY app/Cargo.toml ./Cargo.toml
COPY app/src ./src
COPY app/migrations ./migrations
COPY app/certs/rds-global-bundle.pem ./certs/rds-global-bundle.pem
COPY contracts/implementation-lock.json /source/contracts/implementation-lock.json
# contracts/schema.json is embedded by commands/tests.rs and managed_qualification.rs (APP-013).
COPY contracts/schema.json /source/contracts/schema.json
COPY contracts/fixtures /source/contracts/fixtures
COPY spikes/APP-006/Cargo.toml /source/spikes/APP-006/Cargo.toml
COPY spikes/APP-006/src /source/spikes/APP-006/src
# The executable path comes from Cargo's artifact metadata (the library's
# test-profile artifact), never a guessed filename. ADR-011 (APP-092): the
# cfg(test) canvas campaign derives rust_embed over admin/dist, whose derive
# refuses a missing folder at compile time; this image builds no browser
# bundle and never runs that campaign, so the folder exists empty here.
RUN --mount=type=cache,id=genui-qualification-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=genui-qualification-target-${TARGETARCH},target=/source/target \
    set -eu; \
    mkdir -p /source/app/admin/dist; \
    cargo test --no-run --locked --lib -p forma-app-platform --message-format=json > /source/cargo-artifacts.jsonl; \
    grep '"reason":"compiler-artifact"' /source/cargo-artifacts.jsonl \
      | grep '"name":"genui"' | grep '"kind":\["lib"\]' | grep '"test":true' \
      | grep -o '"executable":"[^"]*"' | cut -d'"' -f4 > /source/executable-path; \
    test "$(wc -l < /source/executable-path)" -eq 1; \
    install -m 0555 "$(cat /source/executable-path)" /source/genui-storage-tests; \
    printf '{"schema_version":1,"executable":"/app/qualification/genui-storage-tests","executable_sha256":"%s","rustc":"%s","cargo_artifact":"%s"}\n' \
      "$(sha256sum /source/genui-storage-tests | cut -d' ' -f1)" \
      "$(rustc --version)" \
      "$(basename "$(cat /source/executable-path)")" > /source/build-identity.json

FROM python:3.11.14-slim-bookworm@sha256:65a93d69fa75478d554f4ad27c85c1e69fa184956261b4301ebaf6dbb0a3543d
# Exact Bookworm security updates, checked against Debian's tracker and ARM64 APT metadata.
# Preserve APP-078's critical fixes; APP-140 adds every available fix from the 2026-11-01 scan.
# Remaining vendor-open findings and package evidence: evidence/APP-140/IMAGE-SECURITY.md.
RUN apt-get update \
    && apt-get install -y --no-install-recommends --only-upgrade \
       libssl3=3.0.22-1~deb12u2 openssl=3.0.22-1~deb12u2 libgnutls30=3.7.9-2+deb12u7 perl-base=5.36.0-7+deb12u4 \
       libc6=2.36-9+deb12u14 libc-bin=2.36-9+deb12u14 dpkg=1.21.23 libcap2=1:2.66-4+deb12u3+b1 \
       libpcre2-8-0=10.42-1+deb12u2 libkrb5-3=1.20.1-2+deb12u5 libk5crypto3=1.20.1-2+deb12u5 \
       libgssapi-krb5-2=1.20.1-2+deb12u5 libkrb5support0=1.20.1-2+deb12u5 \
       libsystemd0=252.39-1~deb12u2 libudev1=252.39-1~deb12u2 libgcrypt20=1.10.1-3+deb12u1 \
       liblzma5=5.4.1-1+deb12u2 sed=4.9-1+deb12u1 \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY app/runtime/requirements.lock ./requirements.lock
RUN python -m pip install --no-cache-dir --require-hashes --only-binary=:all: -r requirements.lock
COPY --from=native --chmod=0555 /source/genui-storage-tests /app/qualification/genui-storage-tests
COPY --from=native --chmod=0444 /source/build-identity.json /app/qualification/build-identity.json
COPY --chmod=0444 app/runtime/runtime.py /app/runtime.py
COPY --chmod=0444 app/runtime/qualification_supervisor.py /app/qualification_supervisor.py
COPY --chmod=0444 app/certs/rds-global-bundle.pem /etc/ssl/certs/rds-global-bundle.pem
RUN mkdir -p /run/genui && chown 65532:65532 /run/genui && chmod 0700 /run/genui
VOLUME ["/run/genui"]
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 AWS_CONFIG_FILE=/dev/null AWS_SHARED_CREDENTIALS_FILE=/dev/null
USER 65532:65532
ENTRYPOINT ["python3", "/app/qualification_supervisor.py"]
CMD ["run"]
