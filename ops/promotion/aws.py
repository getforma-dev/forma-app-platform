"""Bounded AWS transport; no credentials or raw responses are printed."""
import json
import time
from .guards import ACCOUNT, REGION
from .state import require


class AWS:
    def __init__(self):
        import boto3
        from botocore.config import Config
        self.session = boto3.Session(region_name=REGION)
        self.config = Config(connect_timeout=5, read_timeout=60, retries={'total_max_attempts': 1})
        self.clients = {}

    def client(self, name):
        if name not in self.clients:
            self.clients[name] = self.session.client(name, config=self.config)
        return self.clients[name]

    def call(self, service, operation, /, **kwargs):
        return getattr(self.client(service), operation)(**kwargs)

    def identity(self):
        value = self.call('sts', 'get_caller_identity')
        require(value['Account'] == ACCOUNT, 'AWS account differs')
        return value

    def wait(self, service, operation, predicate, timeout=900, retry_codes=frozenset(), **kwargs):
        deadline = time.monotonic() + timeout
        while True:
            try:
                result = self.call(service, operation, **kwargs)
            except Exception as error:
                if getattr(error, 'response', {}).get('Error', {}).get('Code') not in retry_codes:
                    raise
            else:
                if predicate(result):
                    return result
            require(time.monotonic() < deadline, 'bounded wait expired; retain lease and reconcile')
            time.sleep(10)


def body(template):
    return json.loads(template) if isinstance(template, str) else template
