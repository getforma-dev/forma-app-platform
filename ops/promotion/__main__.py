"""Live entrypoint. Detailed failures stay private; stdout is closed summary only."""
import argparse
import json
import sys
from pathlib import Path
from .aws import AWS
from .runner import Runner
from .state import Store, no_duplicates
from .github import validate_summary


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument('phase', choices=['prepare', 'execute'])
    parser.add_argument('--request', type=Path, required=True)
    parser.add_argument('--artifact-dir', type=Path, required=True)
    parser.add_argument('--state-bucket', required=True)
    parser.add_argument('--state-prefix', required=True)
    parser.add_argument('--approval', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(argv)
    if (args.phase == 'execute') != bool(args.approval):
        parser.error('--approval is required only for execute')
    try:
        aws = AWS(); store = Store(aws.client('s3'), args.state_bucket, args.state_prefix)
        request = json.loads(args.request.read_text(), object_pairs_hook=no_duplicates)
        runner = Runner(aws, store)
        if args.phase == 'prepare':
            summary = runner.prepare(request, args.artifact_dir)
        else:
            summary = runner.execute(request, args.artifact_dir, json.loads(args.approval.read_text(), object_pairs_hook=no_duplicates))
        validate_summary(summary, request['candidate_sha'])
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(summary, indent=2) + '\n')
        print(json.dumps(summary))
        return 0
    except Exception:
        # Exception strings may contain signed URLs, private resource identifiers,
        # SDK bodies or state. Do not print them into a public repository's logs.
        print('Promotion refused or outcome unresolved. The private lease and journal are retained; inspect them before any retry.', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
