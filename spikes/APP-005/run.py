"""Build a curated, credential-free image and execute the bounded qualification."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import uuid

from fixture import prepare

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]


def run(output):
    output = Path(output).resolve(); output.mkdir(parents=True, exist_ok=True)
    run_id = uuid.uuid4().hex[:12]
    scratch_parent = HERE / ".scratch"; scratch_parent.mkdir(exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix="qualification-", dir=scratch_parent))
    root = scratch / "org"; context = scratch / "image"; context.mkdir()
    manifest = prepare(root)
    (scratch / "canary.csv").write_text("secret\n987654321\n")
    shutil.copyfile(root / "s02.parquet", scratch / "canary-secret.parquet")
    # Only these explicit sources enter Docker's context. No user home, repository
    # checkout, credentials, unrelated data or environment files are mounted.
    for name in ("Dockerfile", "requirements.lock", "fixture.py", "query.py", "worker.py", "processes.py", "test_processes.py"):
        shutil.copyfile(HERE / name, context / name)
    (context / "contracts").mkdir()
    for name in ("schema.json", "validate.py", "requirements.lock"):
        shutil.copyfile(REPO / "contracts" / name, context / "contracts" / name)
    shutil.copyfile(scratch / "canary.csv", context / "canary.csv")
    shutil.copyfile(scratch / "canary-secret.parquet", context / "canary-secret.parquet")
    name = "forma-app005-" + run_id
    image = "forma-app005-qualification:" + run_id
    build = subprocess.run(["docker", "build", "--tag", image, str(context)], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    (output / "build.log").write_text(build.stdout)
    if build.returncode:
        raise SystemExit("Qualification image build failed; retained build.log")
    image_info = json.loads(subprocess.check_output(["docker", "image", "inspect", image], text=True))[0]
    image_id = image_info["Id"]
    command = ["docker", "create", "--name", name, "--network=none", "--read-only", "--user=65532:65532", "--cap-drop=ALL", "--security-opt=no-new-privileges",
               "--cpus=2", "--memory=3g", "--memory-swap=3g", "--pids-limit=64", "--tmpfs", "/tmp:rw,noexec,nosuid,nodev,size=2147483648,uid=65532,gid=65532,mode=700",
               "--mount", f"type=bind,source={root},target=/data/org,readonly", image_id]
    container_id = subprocess.check_output(command, text=True).strip()
    configuration = json.loads(subprocess.check_output(["docker", "inspect", container_id], text=True))[0]
    host = configuration["HostConfig"]
    assert host["NetworkMode"] == "none" and host["ReadonlyRootfs"] and host["Memory"] == 3 * 1024**3 and host["PidsLimit"] == 64
    assert host["NanoCpus"] == 2 * 10**9 and host["CapDrop"] == ["ALL"]
    assert len(configuration["Mounts"]) == 1 and configuration["Mounts"][0]["RW"] is False
    started = time.monotonic()
    try:
        result = subprocess.run(["docker", "start", "--attach", container_id], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
    except subprocess.TimeoutExpired as exc:
        subprocess.run(["docker", "kill", container_id], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        (output / "failure.txt").write_text("Whole qualification exceeded outer 90-second campaign backstop; owned container killed.\n" + str(exc))
        raise
    finally:
        inspection = json.loads(subprocess.check_output(["docker", "inspect", container_id], text=True))[0]
        (output / "container-state.json").write_text(json.dumps({"state": inspection["State"], "image": image_id, "architecture": image_info["Architecture"], "host_config": {k: host[k] for k in ("NetworkMode", "ReadonlyRootfs", "Memory", "MemorySwap", "NanoCpus", "PidsLimit", "CapDrop", "SecurityOpt", "Tmpfs")}, "mounts": [{"destination": x["Destination"], "read_write": x["RW"]} for x in inspection["Mounts"]], "elapsed_seconds": time.monotonic() - started}, indent=2) + "\n")
        subprocess.run(["docker", "rm", "--force", container_id], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    (output / "stderr.log").write_text(result.stderr)
    try:
        report = json.loads(result.stdout)
    except json.JSONDecodeError:
        (output / "failure.txt").write_text(result.stdout + "\n" + result.stderr)
        raise
    (output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    artifacts = {str(path.relative_to(REPO)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(HERE.glob("*")) if path.is_file()}
    artifacts.update({"contracts/implementation-lock.json": hashlib.sha256((REPO / "contracts/implementation-lock.json").read_bytes()).hexdigest()})
    (output / "source-identity.json").write_text(json.dumps({"recorded_on": "2026-10-28", "image_id": image_id, "base_image": "python:3.11.14-slim-bookworm@sha256:65a93d69fa75478d554f4ad27c85c1e69fa184956261b4301ebaf6dbb0a3543d", "artifacts": artifacts}, indent=2) + "\n")
    print(json.dumps({"passed": report["passed"], "image_id": image_id, "output": str(output), "exit_code": result.returncode}))
    # This task owns these disposable fixtures and this tagged image only.
    shutil.rmtree(scratch)
    subprocess.run(["docker", "image", "rm", image], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return result.returncode if result.returncode else (0 if report["passed"] else 1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(); parser.add_argument("--output", type=Path, default=REPO / "evidence/APP-005/container")
    raise SystemExit(run(parser.parse_args().output))
