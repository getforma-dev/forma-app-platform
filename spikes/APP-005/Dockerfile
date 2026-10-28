FROM python:3.11.14-slim-bookworm@sha256:65a93d69fa75478d554f4ad27c85c1e69fa184956261b4301ebaf6dbb0a3543d
WORKDIR /app
COPY requirements.lock /app/requirements.lock
COPY contracts/requirements.lock /app/contracts/requirements.lock
RUN python -m pip install --no-cache-dir --require-hashes --only-binary=:all: -r /app/requirements.lock -r /app/contracts/requirements.lock
COPY contracts /app/contracts
COPY fixture.py query.py worker.py processes.py /app/
COPY canary.csv /protected/canary.csv
COPY canary-secret.parquet /data/canary-secret.parquet
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 FORMA_CONTRACTS=/app/contracts FORMA_QUERY_TMP=/tmp
USER 65532:65532
ENTRYPOINT ["python", "/app/worker.py", "/data/org", "--container"]
