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
COPY app/Cargo.toml app/Cargo.lock ./
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
# test-profile artifact), never a guessed filename.
RUN --mount=type=cache,id=genui-qualification-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=genui-qualification-target-${TARGETARCH},target=/source/app/target \
    set -eu; \
    cargo test --no-run --locked --lib --message-format=json > /source/cargo-artifacts.jsonl; \
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
