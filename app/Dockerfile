FROM node:22.21.0-bookworm-slim@sha256:f9f7f95dcf1f007b007c4dcd44ea8f7773f931b71dc79d57c216e731c87a090b AS browser
WORKDIR /source/admin
RUN npm install --global npm@11.6.2
COPY admin/package.json admin/package-lock.json ./
RUN npm ci
COPY admin/build.ts admin/tsconfig.json ./
COPY admin/src ./src
RUN npm run typecheck && npm run build

FROM rust:1.93.1-bookworm@sha256:7c4ae649a84014c467d79319bbf17ce2632ae8b8be123ac2fb2ea5be46823f31 AS native
WORKDIR /source
ENV CARGO_BUILD_JOBS=2
ARG TARGETARCH
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY --from=browser /source/admin/dist ./admin/dist
RUN --mount=type=cache,id=genui-cargo-registry,target=/usr/local/cargo/registry --mount=type=cache,id=genui-cargo-target-${TARGETARCH},target=/source/target cargo build --release --locked && cp /source/target/release/forma-app-platform /source/genui

FROM python:3.11.14-slim-bookworm@sha256:65a93d69fa75478d554f4ad27c85c1e69fa184956261b4301ebaf6dbb0a3543d
WORKDIR /app
COPY runtime/requirements.lock ./requirements.lock
RUN python -m pip install --no-cache-dir --require-hashes --only-binary=:all: -r requirements.lock
COPY --from=native /source/genui /app/genui
COPY runtime/runtime.py /app/runtime.py
COPY migrations /app/migrations
COPY certs/rds-global-bundle.pem /etc/ssl/certs/rds-global-bundle.pem
RUN mkdir -p /run/genui && chown 65532:65532 /run/genui && chmod 0700 /run/genui
VOLUME ["/run/genui"]
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 GENUI_RUNTIME_CONFIG_FILE=/run/genui/credentials.json GENUI_RDS_CA_FILE=/etc/ssl/certs/rds-global-bundle.pem AWS_CONFIG_FILE=/dev/null AWS_SHARED_CREDENTIALS_FILE=/dev/null
USER 65532:65532
EXPOSE 3000
ENTRYPOINT ["python3", "/app/runtime.py"]
CMD ["serve"]
