"""The beta deployment's fixed identity, shared by the infra operator scripts.

preflight.py, window_requests.py and readonly_snapshot.py load this file by
its path beside them (importlib), never through ``sys.path``, so they also
run under ``python3 -I``. Values only: no AWS call, no I/O.
"""

ACCOUNT = '877573132739'
REGION = 'us-east-1'
HOSTNAME = 'beta.getforma.dev'
STACK = 'GenuiSyntheticFoundation'
