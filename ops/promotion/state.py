"""One S3 CAS document owns the ledger, deployment lease and write-ahead journal.

A lease never expires automatically. Cancellation, failed writes and unknown AWS
outcomes retain it. Administrative recovery must fence/terminate the original
workflow and its credentials before changing ownership; there is no force-unlock.
"""
import copy
import hashlib
import json
import secrets
from datetime import datetime, timezone

MAX_STATE_BYTES = 8 * 1024 * 1024


class Refused(RuntimeError):
    pass


class LeaseHeld(Refused):
    """A read confirmed another active promotion; acquisition made no write."""


def require(condition, reason):
    if not condition:
        raise Refused(reason)


def encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), default=str) + '\n').encode()


def digest(value):
    return hashlib.sha256(encoded(value)).hexdigest()


def now():
    return datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')


def closed(value, keys, label):
    require(isinstance(value, dict) and set(value) == set(keys), label + ' fields differ')


def no_duplicates(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON field')
        result[key] = value
    return result


class Store:
    def __init__(self, s3, bucket, prefix):
        require(bucket and '/' not in bucket, 'invalid state bucket')
        require(prefix and not prefix.startswith('/') and '..' not in prefix.split('/'), 'invalid state prefix')
        self.s3, self.bucket, self.prefix = s3, bucket, prefix.rstrip('/')
        self.document = self.etag = None
        self.owner = None

    @property
    def key(self):
        return self.prefix + '/state.json'

    def load(self):
        result = self.s3.get_object(Bucket=self.bucket, Key=self.key)
        raw = result['Body'].read(MAX_STATE_BYTES + 1)
        require(len(raw) <= MAX_STATE_BYTES, 'state exceeds bound')
        document = json.loads(raw, object_pairs_hook=no_duplicates)
        require(document.get('schema_version') == 1 and type(document.get('revision')) is int, 'state version differs')
        self.document, self.etag = document, result['ETag']
        return copy.deepcopy(document)

    def save(self, document):
        require(self.document is not None and self.etag, 'state not loaded')
        require(document['revision'] == self.document['revision'] + 1, 'state revision must advance exactly once')
        if self.owner:
            require(self.document.get('active', {}).get('owner') == self.owner, 'deployment lease lost')
        raw = encoded(document)
        require(len(raw) <= MAX_STATE_BYTES, 'state exceeds bound')
        # No retry: 409/412 or a lost response may mean another revision exists.
        result = self.s3.put_object(Bucket=self.bucket, Key=self.key, Body=raw,
                                    ContentType='application/json', ServerSideEncryption='AES256', IfMatch=self.etag)
        require(result.get('ETag'), 'state write outcome unknown')
        self.document, self.etag = copy.deepcopy(document), result['ETag']

    def change(self, update):
        document = copy.deepcopy(self.document)
        update(document)
        document['revision'] += 1
        self.save(document)

    def acquire(self, promotion_id, authorization, candidate):
        self.load()
        if self.document.get('active') is not None:
            raise LeaseHeld('another promotion or unresolved outcome holds the lease')
        owner = secrets.token_hex(32)
        self.change(lambda d: d.update(active={'promotion_id': promotion_id, 'owner': owner,
                    'authorization': authorization, 'candidate_sha': candidate, 'started_at_utc': now(),
                    'phase': 'preparing', 'journal': [], 'packet': None}))
        self.owner = owner

    def resume(self, promotion_id, authorization):
        self.load()
        active = self.document.get('active')
        require(active and active['promotion_id'] == promotion_id, 'promotion lease differs')
        # A separately approved execute job is within the same workflow run.
        for field in ['repository', 'workflow_ref', 'run_id', 'run_attempt']:
            require(active['authorization'][field] == authorization[field], 'workflow provenance differs')
        self.owner = active['owner']
        require(not any(x['status'] == 'intent' for x in active['journal']),
                'unknown operation retained; reconcile original call before resuming')

    def assert_owned(self):
        owner = self.owner
        self.load()
        require(owner and self.document.get('active') and self.document['active']['owner'] == owner,
                'deployment lease differs')
        self.owner = owner

    def immutable(self, name, value):
        require(name and '..' not in name.split('/') and not name.startswith('/'), 'invalid receipt name')
        raw = encoded(value)
        self.s3.put_object(Bucket=self.bucket, Key=self.prefix + '/' + name, Body=raw,
                           ContentType='application/json', ServerSideEncryption='AES256', IfNoneMatch='*')
        return hashlib.sha256(raw).hexdigest()

    def operation(self, name, request, call):
        self.assert_owned()
        active = self.document['active']
        require(not any(x['name'] == name for x in active['journal']), 'operation already journaled; do not repeat')
        require(not any(x['status'] == 'intent' for x in active['journal']), 'prior operation outcome unknown')
        entry = {'name': name, 'request': request, 'request_sha256': digest(request), 'status': 'intent',
                 'intended_at_utc': now()}
        self.change(lambda d: d['active']['journal'].append(entry))
        # Durable CAS intent precedes every mutation. An exception intentionally
        # leaves status=intent and the lease; never infer that the API did nothing.
        result = call()
        self.assert_owned()
        path = 'promotions/' + active['promotion_id'] + '/' + name + '.json'
        receipt_hash = self.immutable(path, {'request': request, 'response': result, 'observed_at_utc': now()})
        def finish(document):
            entry = document['active']['journal'][-1]
            require(entry['name'] == name and entry['status'] == 'intent', 'journal changed')
            entry.update(status='confirmed', receipt=path, receipt_sha256=receipt_hash)
        self.change(finish)
        return result
