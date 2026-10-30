# APP-077 operator image for the separate migration identity (ADR-005,
# ADR-007 D7.19). Repository-root context restricted by
# app/operator/Dockerfile.dockerignore (BuildKit's per-Dockerfile ignore file),
# so the service and qualification contexts are unchanged. Never the service
# image and never behind the load balancer. It holds the canonical migration
# runner and catalog verifier, the seed and contract validator with their exact
# fixtures, the admission operator channel, the fixture source and the
# reviewed APP-004/005 adapter (parser-host and WASM guest built from parser/
# exactly as the service image builds them, DuckDB 2.0.0), the pinned RDS CA
# and hash-locked dependencies. Its entry point takes closed subcommands only.

# Export-only stage: `--target inputs --output type=local,dest=DIR` writes
# the exact build inputs for review; the final image never contains it.
FROM scratch AS inputs
COPY . /

# The service image's parser stage (app/Dockerfile), with the manifest naming
# this image's paths. Same sources, lockfile, toolchain and fixed /source/parser
# path, so parser-manifest.json binds the bytes to parser/ (ADR-009 D9.1).
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
        --guest /source/parser-out/parser_guest.wasm --guest-path /operator/bin/parser_guest.wasm \
        --host /source/parser-out/parser-host --host-path /operator/bin/parser-host \
        --rustc-vv /source/rustc-vv.txt --output /source/parser-out/parser-manifest.json

FROM python:3.11.14-slim-bookworm@sha256:65a93d69fa75478d554f4ad27c85c1e69fa184956261b4301ebaf6dbb0a3543d
WORKDIR /operator
COPY app/operator/requirements.lock /tmp/operator-requirements.lock
RUN python -m pip install --no-cache-dir --require-hashes --only-binary=:all: -r /tmp/operator-requirements.lock \
    && rm /tmp/operator-requirements.lock
# Repository-relative layout under /operator, so the packaged modules resolve
# their siblings exactly as in the checkout (tenant_seed -> contracts/,
# runtime -> app/migrations/, qualify -> verify -> fixtures/phase-one/v1/).
COPY --chmod=0444 app/operator/operator.py app/operator/overrides.json app/operator/requirements.lock /operator/app/operator/
COPY --chmod=0444 app/runtime/runtime.py app/runtime/tenant_seed.py app/runtime/admission_operator.py app/runtime/provider_management.py /operator/app/runtime/
COPY --chmod=0444 app/migrations/ /operator/app/migrations/
COPY --chmod=0444 contracts/validate.py contracts/schema.json /operator/contracts/
COPY --chmod=0444 contracts/fixtures/impl-2026-10-28.1/northstar.json contracts/fixtures/impl-2026-10-28.1/spend-review.json /operator/contracts/fixtures/impl-2026-10-28.1/
COPY --chmod=0444 fixtures/qualify.py fixtures/verify.py /operator/fixtures/
COPY --chmod=0444 fixtures/phase-one/v1/manifest.json /operator/fixtures/phase-one/v1/
COPY --chmod=0444 fixtures/phase-one/v1/sources/northstar-events.csv /operator/fixtures/phase-one/v1/sources/
COPY --from=parser --chmod=0555 /source/parser-out/parser-host /source/parser-out/parser_guest.wasm /operator/bin/
COPY --from=parser --chmod=0444 /source/parser-out/parser-manifest.json /operator/bin/parser-manifest.json
COPY --chmod=0444 app/certs/rds-global-bundle.pem /etc/ssl/certs/rds-global-bundle.pem
# Build identity: the digest of every packaged file, recorded inside the image.
RUN python3 -I -c "import hashlib,json,pathlib; r=pathlib.Path('/operator'); print(json.dumps({'schema_version':1,'files':{str(p.relative_to(r)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(r.rglob('*')) if p.is_file()}},sort_keys=True))" > /tmp/build-identity.json \
    && install -m 0444 /tmp/build-identity.json /operator/build-identity.json && rm /tmp/build-identity.json \
    && find /operator -type d -exec chmod 0555 {} + \
    && mkdir -p /run/operator && chown 65532:65532 /run/operator && chmod 0700 /run/operator
# Private scratch for the parser config and Parquet output; the task's root
# filesystem is read-only.
VOLUME ["/run/operator"]
# The entry point runs isolated (-I), which ignores every PYTHON* variable, so
# -u carries the unbuffered output: a receipt printed after a commit reaches the
# awslogs pipe at once rather than at exit (APP-077 review m6). The PYTHON*
# values below reach only a python3 run without -I.
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 TMPDIR=/run/operator AWS_CONFIG_FILE=/dev/null AWS_SHARED_CREDENTIALS_FILE=/dev/null
USER 65532:65532
ENTRYPOINT ["python3", "-I", "-u", "/operator/app/operator/operator.py"]
CMD ["verify-catalog"]
