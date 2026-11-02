"""Read-only capture and explicit one-time canonical-state cutover.

Never upload the private operator directory wholesale: it contains invitation
capabilities and mailboxes. Only the ledger and allowlisted deployment boundary
are imported. Original local bytes remain as recovery evidence, not a second writer.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
from .aws import AWS, body
from .candidate import deployment_contract_digest
from .guards import ACCOUNT, REGION, ROLE_KEYS, REPOS, outputs, parameters
from .state import Store, encoded, no_duplicates, now, require


def read(path):
    return json.loads(path.read_text(), object_pairs_hook=no_duplicates)


def capture(private, repo, aws):
    ledger_raw = (private / 'budget-ledger.json').read_bytes(); ledger = json.loads(ledger_raw)
    candidates = []
    for wid, entry in ledger['windows'].items():
        if entry.get('activation_requested_at_utc') and not entry.get('superseded_by_window_id') and not entry.get('abandoned_at_utc'):
            candidates.append(wid)
    require(len(candidates) == 1, 'one actual activated ledger head required')
    wid = candidates[0]; window = private / 'windows' / wid; release = read(window / 'release.json')
    require(release['window_id'] == wid and release['verified'].get('live'), 'live release not verified')
    aws.identity(); prior = read(window / 'stack-after.json')['Stacks'][0]
    stack = aws.call('cloudformation', 'describe_stacks', StackName=prior['StackId'])['Stacks'][0]
    require(stack['StackStatus'] == 'UPDATE_COMPLETE' and parameters(stack) == parameters(prior), 'live stack changed')
    out = outputs(stack); require(out == outputs(prior), 'stack outputs changed')
    template = body(aws.call('cloudformation', 'get_template', StackName=stack['StackId'], TemplateStage='Original')['TemplateBody'])
    require(template == read(window / 'template.json'), 'template changed')
    definitions = {key: aws.call('ecs', 'describe_task_definition', taskDefinition=out[key])['taskDefinition'] for key in ROLE_KEYS}
    services = aws.call('ecs', 'describe_services', cluster=out['ClusterName'], services=[out['ServiceName'], out['SharedBrokerServiceName']])
    require(not services.get('failures') and len(services['services']) == 2, 'service inventory incomplete')
    for service in services['services']:
        require((service['desiredCount'], service['runningCount'], service['pendingCount']) == (1, 1, 0), 'services not healthy')
    group, name = out['DeploymentExpiryScheduleArn'].split(':schedule/')[1].split('/')
    schedules = {name: aws.call('scheduler', 'get_schedule', GroupName=group, Name=name)
                 for name in [name, out['SharedBrokerExpiryScheduleName']]}
    for value in schedules.values(): value.pop('ResponseMetadata', None)
    registry = {}
    for role, image in release['images'].items():
        registry_image = aws.call('ecr', 'batch_get_image', repositoryName=REPOS[role], imageIds=[{'imageDigest': image['digest']}],
                                 acceptedMediaTypes=['application/vnd.oci.image.index.v1+json', 'application/vnd.oci.image.manifest.v1+json'])
        require(len(registry_image.get('images', [])) == 1 and not registry_image.get('failures'), 'registry image missing')
        raw = registry_image['images'][0]['imageManifest'].encode(); require('sha256:' + hashlib.sha256(raw).hexdigest() == image['digest'], 'registry index bytes differ')
        doc = json.loads(raw)
        if doc['mediaType'] == 'application/vnd.oci.image.index.v1+json':
            arms = [x for x in doc['manifests'] if x.get('platform', {}).get('architecture') == 'arm64' and x.get('platform', {}).get('os') == 'linux']
            require(len(arms) == 1, 'one ARM64 child required'); child = arms[0]['digest']
            observed = aws.call('ecr', 'batch_get_image', repositoryName=REPOS[role], imageIds=[{'imageDigest': child}], acceptedMediaTypes=['application/vnd.oci.image.manifest.v1+json'])
            child_raw = observed['images'][0]['imageManifest'].encode(); require('sha256:' + hashlib.sha256(child_raw).hexdigest() == child, 'registry child bytes differ')
            child_doc = json.loads(child_raw)
        else:
            child = image['digest']; child_doc = doc
        scan = aws.call('ecr', 'describe_image_scan_findings', repositoryName=REPOS[role], imageId={'imageDigest': child})
        require(scan['imageScanStatus']['status'] == 'COMPLETE', 'baseline scan incomplete')
        findings = list(scan['imageScanFindings'].get('findings', [])); token = scan.get('nextToken')
        while token:
            page = aws.call('ecr', 'describe_image_scan_findings', repositoryName=REPOS[role], imageId={'imageDigest': child}, nextToken=token)
            findings += page['imageScanFindings'].get('findings', []); token = page.get('nextToken')
        registry[role] = {'repository': REPOS[role], 'index_digest': image['digest'], 'manifest_digest': child,
                          'config_digest': child_doc['config']['digest'], 'findings': findings}
    candidate = release['candidate_sha']
    schema_raw = subprocess.check_output(['git', '-C', str(repo), 'show', candidate + ':app/migrations/manifest.json'])
    require(json.loads(schema_raw)['database_version'] == release['database_version'], 'schema release differs')
    inventory = {'account': ACCOUNT, 'region': REGION, 'stack_id': stack['StackId'],
                 'cluster_arn': services['services'][0]['clusterArn'], 'definitions': definitions,
                 'services': services['services'], 'schedules': list(schedules.values()),
                 'target_groups': sorted({lb['targetGroupArn'] for s in services['services'] for lb in s['loadBalancers']})}
    state = {'schema_version': 1, 'revision': 0, 'ledger': ledger, 'active': None, 'completed': {},
        'policy': {'shorten_expiry_hours': 2, 'schema_version': release['database_version'], 'budget_genui_cents': 17000, 'budget_total_cents': 20800},
        'authority': {'stack_id': stack['StackId'], 'prepare_role_arn': f'arn:aws:iam::{ACCOUNT}:role/forma-beta-promotion-prepare',
            'execute_role_arn': f'arn:aws:iam::{ACCOUNT}:role/forma-beta-promotion-execute',
            'execution_role_arn': f'arn:aws:iam::{ACCOUNT}:role/forma-beta-runtime-cloudformation'},
        'baseline': {'template': template, 'definitions': definitions, 'schedules': schedules,
            'schema_manifest_sha256': hashlib.sha256(schema_raw).hexdigest(), 'deployment_contract_sha256': deployment_contract_digest(repo, candidate),
            'clean_trial': read(window / 'clean-trial.json'), 'signin_aliases': read(window / 'signin-aliases.json')},
        'live': {'candidate_sha': candidate, 'window_id': wid, 'config': read(window / 'deployment-input.json'),
                 'parameters': parameters(stack), 'outputs': out, 'registry': registry, 'stack_execution_role_arn': stack['RoleARN']},
        'import': {'observed_at_utc': now(), 'original_ledger_sha256': hashlib.sha256(ledger_raw).hexdigest(),
                   'original_windows': len(ledger['windows']), 'original_ledger_bytes': ledger_raw.decode(),
                   'original_release_sha256': hashlib.sha256((window / 'release.json').read_bytes()).hexdigest()}}
    require((private / 'budget-ledger.json').read_bytes() == ledger_raw, 'ledger changed during capture')
    return {'inventory': inventory, 'state': state}


def install(private, prepared, bucket, prefix, aws):
    require(not (private / 'promotion-managed.json').exists(), 'state already migrated')
    require(hashlib.sha256((private / 'budget-ledger.json').read_bytes()).hexdigest() == prepared['state']['import']['original_ledger_sha256'], 'canonical ledger changed')
    # Capture again at cutover; no changed identity/data/config may be imported
    # merely because an earlier preparation was reviewed.
    actual = capture(private, Path.cwd(), aws)
    for key in ['ledger', 'baseline', 'live', 'authority']:
        require(encoded(actual['state'][key]) == encoded(prepared['state'][key]), 'live state changed before cutover: ' + key)
    s3 = aws.client('s3')
    require(s3.get_bucket_versioning(Bucket=bucket).get('Status') == 'Enabled', 'state bucket must be versioned')
    store = Store(s3, bucket, prefix)
    # Fence legacy canonical env entrypoints BEFORE remote authority is written.
    # A failed/unknown initial PUT deliberately leaves manual entrypoints fenced.
    # Direct historical programs/admin credentials remain an explicit break-glass
    # boundary, not something a local marker can cryptographically revoke.
    env = private / 'env.sh'; backup = private / 'env-before-promotion.sh'
    with backup.open('xb') as stream: stream.write(env.read_bytes())
    backup.chmod(0o600)
    marker = {'schema_version': 1, 'status': 'cutover-intended', 'bucket': bucket, 'prefix': prefix,
              'original_ledger_sha256': prepared['state']['import']['original_ledger_sha256'], 'at_utc': now()}
    (private / 'promotion-managed.json').write_text(json.dumps(marker, indent=2) + '\n')
    env.write_text("printf '%s\\n' 'Legacy beta operations are fenced: canonical state is managed by ops/promotion.' >&2\nexit 96\n")
    env.chmod(0o600)
    response = s3.put_object(Bucket=bucket, Key=store.key, Body=encoded(prepared['state']),
                            ContentType='application/json', ServerSideEncryption='AES256', IfNoneMatch='*')
    observed = store.load(); require(observed == json.loads(encoded(prepared['state'])), 'remote state readback differs')
    marker.update(status='remote-canonical', etag=response['ETag'], at_utc=now())
    (private / 'promotion-managed.json').write_text(json.dumps(marker, indent=2) + '\n')
    (private / 'budget-ledger.json').chmod(0o400)
    return {'status': 'remote-canonical', 'windows_preserved': len(observed['ledger']['windows']), 'ledger_sha256': marker['original_ledger_sha256']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['capture', 'install'])
    parser.add_argument('--private-dir', type=Path, required=True)
    parser.add_argument('--prepared', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--state-bucket')
    parser.add_argument('--state-prefix', default='beta')
    args = parser.parse_args(); aws = AWS()
    require(args.private_dir.is_dir() and args.private_dir.stat().st_mode & 0o077 == 0, 'private owned directory required')
    if args.command == 'capture':
        result = capture(args.private_dir, Path.cwd(), aws)
    else:
        require(args.prepared and args.state_bucket, 'prepared capture and state bucket required')
        result = install(args.private_dir, read(args.prepared), args.state_bucket, args.state_prefix, aws)
    args.output.write_text(json.dumps(result, indent=2, default=str) + '\n'); args.output.chmod(0o600)
    print(json.dumps({'status': 'captured' if args.command == 'capture' else result['status']}))


if __name__ == '__main__':
    main()
