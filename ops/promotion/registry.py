"""Promote validated OCI bytes to existing immutable repositories, never rebuild."""
import hashlib
import json
import re
import tarfile
from datetime import datetime, timezone
from pathlib import Path
from .guards import REPOS
from .state import digest, require

MEDIA = ['application/vnd.oci.image.index.v1+json', 'application/vnd.oci.image.manifest.v1+json']


def publish(aws, store, directory, bundle):
    result = {}
    for role, image in bundle['images'].items():
        repository = REPOS[role]
        repo = aws.call('ecr', 'describe_repositories', repositoryNames=[repository])['repositories'][0]
        require(repo['imageTagMutability'] == 'IMMUTABLE', 'repository tag policy must be immutable')
        require(repo.get('imageScanningConfiguration', {}).get('scanOnPush') is True, 'scan-on-push must be enabled')
        tag = 'promote-' + bundle['candidate_sha']
        existing = aws.call('ecr', 'batch_get_image', repositoryName=repository,
                            imageIds=[{'imageTag': tag}], acceptedMediaTypes=MEDIA)
        require(not existing.get('images') or len(existing['images']) == 1
                and existing['images'][0]['imageId']['imageDigest'] == image['index_digest'], 'conflicting immutable image tag')
        if not existing.get('images'):
            require(len(existing.get('failures', [])) == 1 and existing['failures'][0]['failureCode'] == 'ImageNotFound', 'unknown registry lookup')
            with tarfile.open(directory / image['archive'], 'r') as archive:
                documents = {}
                # Hash every blob in bounded chunks before any layer mutation.
                # Only bounded manifest/config JSON is ever read as one object.
                for blob_digest, size in image['blobs'].items():
                    member = archive.getmember('blobs/sha256/' + blob_digest.removeprefix('sha256:'))
                    require(member.isfile() and member.size == size, 'OCI member shape changed')
                    hashed = hashlib.sha256(); counted = 0
                    with archive.extractfile(member) as stream:
                        while chunk := stream.read(5 * 1024 * 1024):
                            hashed.update(chunk); counted += len(chunk)
                    require(counted == size and 'sha256:' + hashed.hexdigest() == blob_digest, 'OCI bytes changed after validation')
                    value = None; raw = None
                    if size <= 4 * 1024 * 1024:
                        with archive.extractfile(member) as stream: raw = stream.read(size + 1)
                        try: value = json.loads(raw)
                        except (ValueError, UnicodeDecodeError): pass
                    if isinstance(value, dict) and value.get('mediaType') in MEDIA and value.get('schemaVersion') == 2:
                        documents[blob_digest] = (value, raw)
                        continue
                    available = aws.call('ecr', 'batch_check_layer_availability', repositoryName=repository, layerDigests=[blob_digest])
                    if any(x.get('layerDigest') == blob_digest and x.get('layerAvailability') == 'AVAILABLE' for x in available.get('layers', [])):
                        continue
                    label = role + '-' + blob_digest[7:23]
                    opened = store.operation(label + '-begin', {'repositoryName': repository, 'digest': blob_digest},
                        lambda: aws.call('ecr', 'initiate_layer_upload', repositoryName=repository))
                    upload = opened['uploadId']; uploaded = hashlib.sha256(); offset = 0
                    # ECR's minimum part size is 5 MiB, except the final part.
                    with archive.extractfile(member) as stream:
                        while part := stream.read(5 * 1024 * 1024):
                            uploaded.update(part)
                            request = {'repositoryName': repository, 'uploadId': upload, 'partFirstByte': offset,
                                       'partLastByte': offset + len(part) - 1, 'part_sha256': hashlib.sha256(part).hexdigest()}
                            store.operation(label + '-part-' + str(offset), request,
                                lambda request=request, part=part: aws.call('ecr', 'upload_layer_part',
                                    **{k: v for k, v in request.items() if k != 'part_sha256'}, layerPartBlob=part))
                            offset += len(part)
                    require(offset == size and 'sha256:' + uploaded.hexdigest() == blob_digest, 'OCI changed during upload; do not complete')
                    store.operation(label + '-complete', {'repositoryName': repository, 'uploadId': upload, 'digest': blob_digest},
                        lambda: aws.call('ecr', 'complete_layer_upload', repositoryName=repository, uploadId=upload, layerDigests=[blob_digest]))
                placed = set()
                def put(blob_digest, with_tag=False):
                    if blob_digest in placed:
                        return
                    doc, raw = documents[blob_digest]
                    for descriptor in doc.get('manifests', []):
                        put(descriptor['digest'])
                    arguments = {'repositoryName': repository, 'imageManifest': raw.decode(),
                                 'imageManifestMediaType': doc['mediaType'], 'imageDigest': blob_digest}
                    if with_tag:
                        arguments['imageTag'] = tag
                    # Skip an existing exact untagged manifest, but still bind the
                    # final immutable promotion tag via PutImage when absent.
                    found = aws.call('ecr', 'batch_get_image', repositoryName=repository,
                                     imageIds=[{'imageDigest': blob_digest}], acceptedMediaTypes=MEDIA)
                    if not with_tag and found.get('images'):
                        require(found['images'][0]['imageManifest'].encode() == raw, 'remote manifest bytes differ')
                    else:
                        receipt = store.operation(role + '-manifest-' + blob_digest[7:23],
                            {'repositoryName': repository, 'digest': blob_digest, 'tag': tag if with_tag else None,
                             'manifest_sha256': hashlib.sha256(raw).hexdigest()}, lambda: aws.call('ecr', 'put_image', **arguments))
                        require(receipt['image']['imageId']['imageDigest'] == blob_digest, 'registry digest differs')
                    placed.add(blob_digest)
                put(image['index_digest'], True)
        observed = {}
        for name, blob_digest in [('index', image['index_digest']), ('arm64_child', image['manifest_digest'])]:
            response = aws.call('ecr', 'batch_get_image', repositoryName=repository,
                                imageIds=[{'imageDigest': blob_digest}], acceptedMediaTypes=MEDIA)
            require(len(response.get('images', [])) == 1 and not response.get('failures'), 'registry manifest unavailable')
            raw = response['images'][0]['imageManifest'].encode()
            require('sha256:' + hashlib.sha256(raw).hexdigest() == blob_digest, 'registry manifest hash differs')
            observed[name] = json.loads(raw)
        if image['index_digest'] == image['manifest_digest']:
            require(observed['index'].get('mediaType') == MEDIA[1], 'root is not an image manifest')
        else:
            arms = [x for x in observed['index']['manifests'] if x.get('platform', {}).get('architecture') == 'arm64' and x.get('platform', {}).get('os') == 'linux']
            require(len(arms) == 1 and arms[0]['digest'] == image['manifest_digest'], 'registry ARM64 relationship differs')
        require(observed['arm64_child']['config']['digest'] == image['config_digest'], 'registry config relationship differs')
        scan = aws.wait('ecr', 'describe_image_scan_findings', lambda r: scan_complete(r),
                        timeout=600, retry_codes={'ScanNotFoundException'}, repositoryName=repository, imageId={'imageDigest': image['manifest_digest']})
        findings = list(scan.get('imageScanFindings', {}).get('findings', [])); token = scan.get('nextToken')
        while token:
            page = aws.call('ecr', 'describe_image_scan_findings', repositoryName=repository,
                            imageId={'imageDigest': image['manifest_digest']}, nextToken=token)
            findings += page.get('imageScanFindings', {}).get('findings', []); token = page.get('nextToken')
        require(not any(x.get('severity') == 'CRITICAL' for x in findings), 'critical scan finding requires separate reviewed release')
        result[role] = {'repository': repository, 'index_digest': image['index_digest'], 'manifest_digest': image['manifest_digest'],
                        'config_digest': image['config_digest'], 'observed_manifests': observed, 'scan': scan, 'findings': findings}
    return result


def finding_set(findings):
    return {(x['name'], x['severity'], tuple(sorted((a['key'], a['value']) for a in x.get('attributes', [])))) for x in findings}


DISPOSITION = Path(__file__).with_name('scan-disposition.json')
# The reviewed scope is pinned here, in source, so editing the JSON alone can never
# widen it: the evidence record, the roles, and each accepted finding's name and
# severity. A different finding, severity or role needs a reviewed change to this
# constant. The APP-167 operator-only disposition (three MEDIUM gcc-12 findings,
# expiring 2026-11-09) preceded this one; its record stays in evidence/APP-167.
REVIEWED_DISPOSITION = {
    'evidence': 'evidence/APP-173/SCAN-DISPOSITION.md',
    'roles': frozenset(REPOS),
    'findings': {'CVE-2026-76844': 'HIGH', 'CVE-2026-109518': 'HIGH'},
}


def scan_disposition():
    raw = DISPOSITION.read_bytes()
    value = json.loads(raw)
    require(set(value) == {'schema_version', 'deployment_contract_sha256', 'expires_at_utc', 'evidence', 'roles'}
        and value['schema_version'] == 2 and value['evidence'] == REVIEWED_DISPOSITION['evidence']
        and isinstance(value['deployment_contract_sha256'], str)
        and re.fullmatch(r'[0-9a-f]{64}', value['deployment_contract_sha256'])
        and isinstance(value['expires_at_utc'], str)
        and re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z', value['expires_at_utc'])
        and isinstance(value['roles'], dict) and set(value['roles']) == REVIEWED_DISPOSITION['roles'],
        'invalid scan disposition')
    try: datetime.fromisoformat(value['expires_at_utc'].replace('Z', '+00:00'))
    except ValueError: require(False, 'invalid scan disposition')
    for entry in value['roles'].values():
        require(isinstance(entry, dict) and set(entry) == {'predecessor_manifest_digest', 'findings'}
            and isinstance(entry['predecessor_manifest_digest'], str)
            and re.fullmatch(r'sha256:[0-9a-f]{64}', entry['predecessor_manifest_digest'])
            and isinstance(entry['findings'], list), 'invalid scan disposition')
        require(len(entry['findings']) == len(REVIEWED_DISPOSITION['findings'])
            and all(isinstance(f, dict) and set(f) == {'name', 'severity', 'attributes'}
                and REVIEWED_DISPOSITION['findings'].get(f['name']) == f['severity'] for f in entry['findings'])
            and {f['name'] for f in entry['findings']} == set(REVIEWED_DISPOSITION['findings']),
            'disposition cannot broaden severity or scope')
    return value, hashlib.sha256(raw).hexdigest()


def guard_scan_delta(current, baseline, deployment_contract=None, clock=None):
    clock = clock or datetime.now(timezone.utc)
    result = {}
    for role in REPOS:
        require(not any(f.get('severity') == 'CRITICAL' for f in current[role]['findings']),
                'critical scan finding requires separate reviewed release')
        before = finding_set(baseline[role]['findings'])
        introduced = finding_set(current[role]['findings']) - before
        reviewed = []; disposition_sha = None; expiry = None
        if introduced:
            disposition, disposition_sha = scan_disposition()
            entry = disposition['roles'][role]
            require(baseline[role]['manifest_digest'] == entry['predecessor_manifest_digest']
                and deployment_contract == disposition['deployment_contract_sha256']
                and clock < datetime.fromisoformat(disposition['expires_at_utc'].replace('Z', '+00:00')),
                'scan disposition scope changed or expired')
            require(introduced <= finding_set(entry['findings']),
                'new scan findings require separately reviewed disposition: ' + role)
            reviewed = sorted(x[0] for x in introduced)
            require(len(reviewed) == len(set(reviewed)), 'ambiguous reviewed finding')
            expiry = disposition['expires_at_utc']
        result[role] = {'findings': len(current[role]['findings']), 'introduced': len(introduced),
            'reviewed': reviewed, 'reviewed_count': len(reviewed), 'unreviewed': 0, 'disposition_sha256': disposition_sha,
            'review_expires_at_utc': expiry}
    return result


def scan_complete(response):
    status = response['imageScanStatus']['status']
    require(status in ['PENDING', 'IN_PROGRESS', 'COMPLETE'], 'terminal registry scan failure')
    return status == 'COMPLETE'
