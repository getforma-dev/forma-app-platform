"""Pins the standalone security templates (ops/monitoring/security-events.json and waf.json): the free resource set, the
rule patterns, the rate limits and their scope, and that neither template touches the deployment contract in infra/src."""
import json
from pathlib import Path
import re
import unittest

MONITORING = Path(__file__).resolve().parents[1] / 'monitoring'
ROOT = MONITORING.parents[1]
EVENTS = MONITORING / 'security-events.json'
WAF = MONITORING / 'waf.json'
EVENTS_TOPIC = {'Ref': 'SecurityTopic'}
TRAIL_ARN = {'Fn::Sub': 'arn:${AWS::Partition}:cloudtrail:${AWS::Region}:${AWS::AccountId}:trail/${AWS::StackName}-management'}
API_CALL = 'AWS API Call via CloudTrail'
SIGN_IN = 'AWS Console Sign In via CloudTrail'
NO_TRANSFORM = [{'Priority': 0, 'Type': 'NONE'}]


def strings(value):
    if isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)
    elif isinstance(value, str):
        yield value


def event_names(pattern):
    """Every eventName a pattern can match, through `$or` and nesting."""
    if isinstance(pattern, dict):
        for key, value in pattern.items():
            if key == 'eventName':
                yield from value
            else:
                yield from event_names(value)
    elif isinstance(pattern, list):
        for item in pattern:
            yield from event_names(item)


def promotion_operations():
    """The AWS operations ops/promotion calls (aws.call / aws.wait), as CloudTrail names them."""
    names = set()
    for source in (ROOT / 'ops' / 'promotion').glob('*.py'):
        if source.name.startswith('test_'):
            continue
        for _, operation in re.findall(r"aws\.(?:call|wait)\(\s*'([a-z0-9-]+)'\s*,\s*'([a-z_]+)'", source.read_text()):
            names.add(''.join(part.capitalize() for part in operation.split('_')))
    return names


def byte_matches(statement):
    """(constraint, search string) of every ByteMatchStatement under a statement, with its nesting depth."""
    def walk(node, depth):
        if 'ByteMatchStatement' in node:
            match = node['ByteMatchStatement']
            yield depth, match['PositionalConstraint'], match['SearchString'], match
        for logical in ('OrStatement', 'AndStatement', 'NotStatement'):
            if logical in node:
                children = node[logical].get('Statements') or [node[logical]['Statement']]
                for child in children:
                    yield from walk(child, depth + 1)
    yield from walk(statement, 0)


class NoPrivateIdentifierMixin:
    def assert_no_private_identifier(self, path, template):
        text = path.read_text()
        for private in ('877573132739', 'dkr.ecr', '/Users/', 'amazonaws.com/', 'Fn::ImportValue'):
            self.assertFalse(private in text, private)
        self.assertIsNone(re.search(r'(?<![0-9])[0-9]{12}(?![0-9])', text.replace('268435456', '')))
        patterns = {p.get('AllowedPattern') for p in template.get('Parameters', {}).values()}
        for value in strings(template):
            if value in patterns:
                continue
            self.assertNotIn('arn:aws:', value, value)
            self.assertNotRegex(value, r'^subnet-|^sg-|^vpc-|^i-')


class SecurityEventsTemplateTests(NoPrivateIdentifierMixin, unittest.TestCase):
    def setUp(self):
        self.template = json.loads(EVENTS.read_text())
        self.resources = self.template['Resources']
        self.rules = {name: r['Properties'] for name, r in self.resources.items() if r['Type'] == 'AWS::Events::Rule'}

    def test_the_only_parameter_is_the_address(self):
        self.assertEqual(self.template['AWSTemplateFormatVersion'], '2010-09-09')
        self.assertEqual(list(self.template['Parameters']), ['NotificationEmail'])
        parameter = self.template['Parameters']['NotificationEmail']
        self.assertEqual(parameter['Type'], 'String')
        self.assertNotIn('Default', parameter)
        self.assertTrue(re.fullmatch(parameter['AllowedPattern'], 'victor@example.test'))
        self.assertFalse(re.fullmatch(parameter['AllowedPattern'], 'not an address'))

    def test_resources_are_the_trail_its_bucket_the_topic_eleven_rules_and_the_analyzer_and_nothing_paid(self):
        types = {name: r['Type'] for name, r in self.resources.items()}
        self.assertEqual(types, {
            'TrailBucket': 'AWS::S3::Bucket', 'TrailBucketPolicy': 'AWS::S3::BucketPolicy', 'ManagementTrail': 'AWS::CloudTrail::Trail',
            'SecurityTopic': 'AWS::SNS::Topic', 'SecurityTopicPolicy': 'AWS::SNS::TopicPolicy', 'EmailSubscription': 'AWS::SNS::Subscription',
            'RootSignInRule': 'AWS::Events::Rule', 'RootApiCallRule': 'AWS::Events::Rule', 'ConsoleSignInFailureRule': 'AWS::Events::Rule',
            'ConsoleSignInWithoutMfaRule': 'AWS::Events::Rule', 'IamChangeRule': 'AWS::Events::Rule', 'CloudTrailTamperRule': 'AWS::Events::Rule',
            'SecurityGroupIngressRule': 'AWS::Events::Rule', 'KmsKeyRule': 'AWS::Events::Rule', 'S3ExposureRule': 'AWS::Events::Rule',
            'RdsExposureRule': 'AWS::Events::Rule', 'AccessAnalyzerFindingRule': 'AWS::Events::Rule',
            'AccountAnalyzer': 'AWS::AccessAnalyzer::Analyzer'})
        self.assertEqual(len(self.rules), 11)
        # Victor's decision 2026-11-15: free observability only. And standalone: nothing promotion protects or the beta stack owns.
        for forbidden in ('AWS::GuardDuty::', 'AWS::SecurityHub::', 'AWS::Config::', 'AWS::Inspector', 'AWS::Logs::', 'AWS::KMS::', 'AWS::IAM::',
                          'AWS::RDS::', 'AWS::ElasticLoadBalancingV2::', 'AWS::ECS::', 'AWS::SecretsManager::', 'AWS::WAFv2::'):
            self.assertFalse(any(t.startswith(forbidden) for t in types.values()), forbidden)
        self.assertEqual(self.resources['AccountAnalyzer']['Properties']['Type'], 'ACCOUNT')
        self.assertEqual(set(self.template['Outputs']), {'TrailBucketName', 'TrailArn', 'SecurityTopicArn', 'AnalyzerArn'})
        self.assertEqual(self.template['Outputs']['SecurityTopicArn']['Value'], EVENTS_TOPIC)

    def test_the_trail_is_the_free_copy_of_management_events_with_validation(self):
        trail = self.resources['ManagementTrail']
        self.assertEqual(trail['DependsOn'], 'TrailBucketPolicy')
        properties = trail['Properties']
        self.assertEqual(properties['TrailName'], {'Fn::Sub': '${AWS::StackName}-management'})
        self.assertEqual(properties['S3BucketName'], {'Ref': 'TrailBucket'})
        self.assertEqual((properties['IsLogging'], properties['IsMultiRegionTrail'], properties['IncludeGlobalServiceEvents'], properties['EnableLogFileValidation']),
                         (True, True, True, True))
        self.assertEqual(properties['EventSelectors'], [{'ReadWriteType': 'All', 'IncludeManagementEvents': True}])
        # No data events, Insights, CloudWatch Logs delivery, KMS key or organization trail: each costs money or needs IAM.
        for paid in ('DataResources', 'AdvancedEventSelectors', 'InsightSelectors', 'CloudWatchLogsLogGroupArn', 'CloudWatchLogsRoleArn', 'KMSKeyId',
                     'IsOrganizationTrail', 'S3KeyPrefix'):
            self.assertNotIn(paid, json.dumps(properties), paid)

    def test_the_bucket_is_private_sse_s3_versioned_tls_only_scoped_to_the_trail_and_retained(self):
        bucket = self.resources['TrailBucket']
        self.assertEqual((bucket['DeletionPolicy'], bucket['UpdateReplacePolicy']), ('Retain', 'Retain'))
        properties = bucket['Properties']
        self.assertNotIn('BucketName', properties)
        self.assertEqual([r['ServerSideEncryptionByDefault']['SSEAlgorithm'] for r in properties['BucketEncryption']['ServerSideEncryptionConfiguration']], ['AES256'])
        self.assertEqual(properties['PublicAccessBlockConfiguration'],
                         {'BlockPublicAcls': True, 'BlockPublicPolicy': True, 'IgnorePublicAcls': True, 'RestrictPublicBuckets': True})
        self.assertEqual(properties['OwnershipControls'], {'Rules': [{'ObjectOwnership': 'BucketOwnerEnforced'}]})
        self.assertEqual(properties['VersioningConfiguration'], {'Status': 'Enabled'})
        [rule] = properties['LifecycleConfiguration']['Rules']
        self.assertEqual(rule['Status'], 'Enabled')
        self.assertEqual(rule['Transitions'], [{'StorageClass': 'GLACIER_IR', 'TransitionInDays': 30}])
        self.assertEqual(rule['ExpirationInDays'], 365)
        self.assertEqual(rule['NoncurrentVersionExpiration'], {'NoncurrentDays': 30})

        policy = self.resources['TrailBucketPolicy']['Properties']
        self.assertEqual(policy['Bucket'], {'Ref': 'TrailBucket'})
        statements = {s['Sid']: s for s in policy['PolicyDocument']['Statement']}
        self.assertEqual(set(statements), {'AWSCloudTrailAclCheck', 'AWSCloudTrailWrite', 'DenyInsecureTransport'})
        acl = statements['AWSCloudTrailAclCheck']
        self.assertEqual((acl['Effect'], acl['Principal'], acl['Action']), ('Allow', {'Service': 'cloudtrail.amazonaws.com'}, 's3:GetBucketAcl'))
        self.assertEqual(acl['Condition'], {'StringEquals': {'aws:SourceArn': TRAIL_ARN}})
        write = statements['AWSCloudTrailWrite']
        self.assertEqual((write['Effect'], write['Principal'], write['Action']), ('Allow', {'Service': 'cloudtrail.amazonaws.com'}, 's3:PutObject'))
        self.assertEqual(write['Resource'], {'Fn::Sub': '${TrailBucket.Arn}/AWSLogs/${AWS::AccountId}/*'})
        self.assertEqual(write['Condition'], {'StringEquals': {'s3:x-amz-acl': 'bucket-owner-full-control', 'aws:SourceArn': TRAIL_ARN}})
        deny = statements['DenyInsecureTransport']
        self.assertEqual((deny['Effect'], deny['Principal'], deny['Action']), ('Deny', '*', 's3:*'))
        self.assertEqual(deny['Condition'], {'Bool': {'aws:SecureTransport': 'false'}})
        self.assertEqual(len(deny['Resource']), 2)

    def test_the_topic_admits_eventbridge_of_this_account_and_mails_the_address(self):
        statement = self.resources['SecurityTopicPolicy']['Properties']['PolicyDocument']['Statement']
        self.assertEqual([(s['Effect'], s['Principal'], s['Action'], s['Resource']) for s in statement],
                         [('Allow', {'Service': 'events.amazonaws.com'}, 'sns:Publish', EVENTS_TOPIC)])
        self.assertEqual(statement[0]['Condition'], {'StringEquals': {'aws:SourceAccount': {'Ref': 'AWS::AccountId'}}})
        subscription = self.resources['EmailSubscription']['Properties']
        self.assertEqual((subscription['Protocol'], subscription['Endpoint'], subscription['TopicArn']), ('email', {'Ref': 'NotificationEmail'}, EVENTS_TOPIC))

    def test_every_rule_is_enabled_named_described_and_mails_one_plain_line(self):
        names = {'RootSignInRule': 'root-sign-in', 'RootApiCallRule': 'root-api-call', 'ConsoleSignInFailureRule': 'console-sign-in-failure',
                 'ConsoleSignInWithoutMfaRule': 'console-sign-in-without-mfa', 'IamChangeRule': 'iam-change', 'CloudTrailTamperRule': 'cloudtrail-tamper',
                 'SecurityGroupIngressRule': 'security-group-ingress', 'KmsKeyRule': 'kms-key', 'S3ExposureRule': 's3-exposure',
                 'RdsExposureRule': 'rds-exposure', 'AccessAnalyzerFindingRule': 'access-analyzer-finding'}
        self.assertEqual(set(self.rules), set(names))
        for name, rule in self.rules.items():
            with self.subTest(rule=name):
                self.assertEqual(rule['State'], 'ENABLED')
                self.assertEqual(rule['Name'], {'Fn::Sub': '${AWS::StackName}-' + names[name]})
                self.assertGreater(len(rule['Description']), 60)
                self.assertNotIn('EventBusName', rule)
                [target] = rule['Targets']
                self.assertEqual(target['Arn'], EVENTS_TOPIC)
                transformer = target['InputTransformer']
                template = transformer['InputTemplate']
                # A quoted string template: SNS mails it as one line; every <key> is mapped from the event.
                self.assertTrue(template.startswith('"') and template.endswith('"') and '\n' not in template, template)
                keys = set(re.findall(r'<([a-z]+)>', template))
                self.assertEqual(keys, set(transformer['InputPathsMap']), name)
                for path in transformer['InputPathsMap'].values():
                    self.assertTrue(path.startswith('$.detail.') or path == '$.time', path)
                self.assertIn('when', keys)
                if name != 'AccessAnalyzerFindingRule':
                    self.assertIn('ip', keys)
                    self.assertIn('$.detail.sourceIPAddress', transformer['InputPathsMap'].values())

    def test_the_rule_patterns_are_exactly_these(self):
        cloudtrail = lambda source, service, names: {'source': [source], 'detail-type': [API_CALL], 'detail': {'eventSource': [service], 'eventName': names}}
        expected = {
            'RootSignInRule': {'source': ['aws.signin'], 'detail-type': [SIGN_IN], 'detail': {'eventName': ['ConsoleLogin'], 'userIdentity': {'type': ['Root']}}},
            # Root's own mutating calls: not an AWS service acting for root, not a read (a billing visit would mail dozens).
            'RootApiCallRule': {'detail-type': [API_CALL], 'detail': {'userIdentity': {'type': ['Root'], 'invokedBy': [{'exists': False}]}, 'readOnly': [False]}},
            'ConsoleSignInFailureRule': {'source': ['aws.signin'], 'detail-type': [SIGN_IN],
                                         'detail': {'eventName': ['ConsoleLogin'], 'responseElements': {'ConsoleLogin': ['Failure']}}},
            # Federated sign-ins carry their MFA at the identity provider and are left out.
            'ConsoleSignInWithoutMfaRule': {'source': ['aws.signin'], 'detail-type': [SIGN_IN],
                                            'detail': {'eventName': ['ConsoleLogin'], 'userIdentity': {'type': ['IAMUser', 'Root']},
                                                       'responseElements': {'ConsoleLogin': ['Success']}, 'additionalEventData': {'MFAUsed': ['No']}}},
            'IamChangeRule': cloudtrail('aws.iam', 'iam.amazonaws.com', [
                'CreateUser', 'CreateAccessKey', 'CreateLoginProfile',
                'AttachUserPolicy', 'PutUserPolicy', 'DetachUserPolicy',
                'AttachRolePolicy', 'PutRolePolicy', 'DetachRolePolicy',
                'AttachGroupPolicy', 'PutGroupPolicy', 'DetachGroupPolicy',
                'UpdateAssumeRolePolicy', 'DeactivateMFADevice', 'DeleteVirtualMFADevice']),
            'CloudTrailTamperRule': cloudtrail('aws.cloudtrail', 'cloudtrail.amazonaws.com', ['StopLogging', 'DeleteTrail', 'UpdateTrail', 'PutEventSelectors']),
            'SecurityGroupIngressRule': cloudtrail('aws.ec2', 'ec2.amazonaws.com', ['AuthorizeSecurityGroupIngress']),
            'KmsKeyRule': cloudtrail('aws.kms', 'kms.amazonaws.com', ['DisableKey', 'ScheduleKeyDeletion']),
            # The CloudTrail names of the public-access-block calls (the brief's DeletePublicAccessBlock/PutPublicAccessBlock are not event names).
            'S3ExposureRule': cloudtrail('aws.s3', 's3.amazonaws.com', [
                'PutBucketPolicy', 'PutBucketAcl',
                'PutBucketPublicAccessBlock', 'DeleteBucketPublicAccessBlock',
                'PutAccountPublicAccessBlock', 'DeleteAccountPublicAccessBlock']),
            'RdsExposureRule': {'source': ['aws.rds'], 'detail-type': [API_CALL], 'detail': {'eventSource': ['rds.amazonaws.com'], '$or': [
                {'eventName': ['DeleteDBInstance', 'DeleteDBSnapshot']},
                {'eventName': ['ModifyDBInstance'], 'requestParameters': {'publiclyAccessible': [True]}}]}},
            'AccessAnalyzerFindingRule': {'source': ['aws.access-analyzer'], 'detail-type': ['Access Analyzer Finding'], 'detail': {'status': ['ACTIVE']}},
        }
        self.assertEqual(set(self.rules), set(expected))
        for name, pattern in expected.items():
            with self.subTest(rule=name):
                self.assertEqual(self.rules[name]['EventPattern'], pattern)

    def test_no_rule_names_an_operation_the_promotion_pipeline_calls(self):
        called = promotion_operations()
        # Sanity: the scan found the pipeline's calls (ECR pushes, ECS updates, change sets, state objects).
        self.assertTrue({'UpdateService', 'RunTask', 'DescribeStacks', 'PutImage', 'DeleteObject', 'PutSecretValue', 'GetSchedule'} <= called, called)
        named = {n for rule in self.rules.values() for n in event_names(rule['EventPattern'])}
        self.assertGreater(len(named), 25)
        self.assertEqual(named & called, set())
        # The pipeline's S3 calls are object (data) events; bucket-level exposure names stay distinct from them.
        self.assertFalse(any(n.endswith('Object') or n.endswith('Objects') for n in named))
        # Task definitions and services change in every release and are not alerted on.
        self.assertFalse(any('TaskDefinition' in n or n.endswith('Service') or n.startswith('ExecuteChangeSet') for n in named))

    def test_no_private_identifier_is_embedded(self):
        self.assert_no_private_identifier(EVENTS, self.template)


class WafTemplateTests(NoPrivateIdentifierMixin, unittest.TestCase):
    MODEL_PREFIXES = {'/v1/compose', '/v1/ask', '/v1/shape', '/v1/mcp', '/v1/act', '/v1/widgets/customize', '/v1/boxes/explain/'}
    MODEL_BOX_SUFFIXES = {'/apps', '/brief', '/ask'}

    def setUp(self):
        self.template = json.loads(WAF.read_text())
        self.resources = self.template['Resources']
        self.acl = self.resources['WebAcl']['Properties']
        self.rules = {rule['Name']: rule for rule in self.acl['Rules']}

    def test_parameters_are_the_two_arns_and_the_two_limits(self):
        self.assertEqual(self.template['AWSTemplateFormatVersion'], '2010-09-09')
        parameters = self.template['Parameters']
        self.assertEqual(set(parameters), {'LoadBalancerArn', 'AlarmTopicArn', 'FloodLimit', 'ModelRouteLimit'})
        alb = parameters['LoadBalancerArn']['AllowedPattern']
        self.assertTrue(re.fullmatch(alb, 'arn:aws:elasticloadbalancing:us-east-1:123456789012:loadbalancer/app/Genui-LoadB-ABCDEF123456/0123456789abcdef'))
        self.assertFalse(re.fullmatch(alb, 'app/Genui-LoadB-ABCDEF123456/0123456789abcdef'))
        self.assertFalse(re.fullmatch(alb, 'arn:aws:elasticloadbalancing:us-east-1:123456789012:loadbalancer/net/x/0123456789abcdef'))
        topic = parameters['AlarmTopicArn']['AllowedPattern']
        self.assertTrue(re.fullmatch(topic, 'arn:aws:sns:us-east-1:123456789012:forma-beta-trial-alarms-AlarmTopic-ABCDEF123456'))
        self.assertFalse(re.fullmatch(topic, 'forma-beta-trial-alarms'))
        for name in ('LoadBalancerArn', 'AlarmTopicArn'):
            self.assertEqual(parameters[name]['Type'], 'String')
            self.assertNotIn('Default', parameters[name])
        self.assertEqual((parameters['FloodLimit']['Type'], parameters['FloodLimit']['Default']), ('Number', 2000))
        self.assertEqual((parameters['ModelRouteLimit']['Type'], parameters['ModelRouteLimit']['Default']), ('Number', 30))
        self.assertLessEqual(10, parameters['ModelRouteLimit']['MinValue'])  # WAF's floor for a rate-based limit.

    def test_resources_are_the_web_acl_its_association_and_two_alarms_with_no_logging_or_managed_groups(self):
        types = {name: r['Type'] for name, r in self.resources.items()}
        self.assertEqual(types, {'WebAcl': 'AWS::WAFv2::WebACL', 'WebAclAssociation': 'AWS::WAFv2::WebACLAssociation',
                                 'FloodBlockedAlarm': 'AWS::CloudWatch::Alarm', 'ModelRoutesBlockedAlarm': 'AWS::CloudWatch::Alarm'})
        text = WAF.read_text()
        self.assertNotIn('LoggingConfiguration"', text.replace('"Logging": "AWS::WAFv2::LoggingConfiguration', ''))
        self.assertNotIn('ManagedRuleGroupStatement"', text)
        self.assertNotIn('AWS::WAFv2::IPSet', text)
        # The paid upgrade path is written down, not deployed.
        upgrade = self.template['Metadata']['UpgradePath']
        self.assertEqual([g['Name'] for g in upgrade['ManagedRuleGroups']],
                         ['AWSManagedRulesAmazonIpReputationList', 'AWSManagedRulesKnownBadInputsRuleSet', 'AWSManagedRulesCommonRuleSet'])
        association = self.resources['WebAclAssociation']['Properties']
        self.assertEqual(association, {'ResourceArn': {'Ref': 'LoadBalancerArn'}, 'WebACLArn': {'Fn::GetAtt': ['WebAcl', 'Arn']}})
        self.assertEqual(self.template['Outputs']['WebAclArn']['Value'], {'Fn::GetAtt': ['WebAcl', 'Arn']})
        self.assertEqual(self.template['Outputs']['WebAclName']['Value'], self.acl['Name'])

    def test_the_web_acl_is_regional_allows_by_default_and_blocks_with_a_429(self):
        self.assertEqual(self.acl['Scope'], 'REGIONAL')
        self.assertEqual(self.acl['DefaultAction'], {'Allow': {}})
        self.assertEqual(self.acl['Name'], {'Fn::Sub': '${AWS::StackName}-web-acl'})
        self.assertEqual(self.acl['VisibilityConfig'], {'SampledRequestsEnabled': True, 'CloudWatchMetricsEnabled': True, 'MetricName': {'Fn::Sub': '${AWS::StackName}-web-acl'}})
        bodies = self.acl['CustomResponseBodies']
        self.assertEqual(list(bodies), ['too-many-requests'])
        self.assertEqual(bodies['too-many-requests']['ContentType'], 'TEXT_HTML')
        # The themed page (APP-217's look, Victor 2026-11-15) is self-contained: a blocked
        # address cannot fetch an asset, so no script, no external resource, and small.
        page = bodies['too-many-requests']['Content']
        self.assertLessEqual(len(page.encode()), 4096)
        self.assertIn('ERROR 429', page)
        self.assertIn('five minutes', page)
        self.assertNotIn('<script', page.lower())
        self.assertNotRegex(page, r'(src|href)="(?!mailto:)')
        self.assertNotIn('url(', page)
        self.assertEqual([(r['Name'], r['Priority']) for r in self.acl['Rules']], [('per-address-flood', 10), ('per-address-model-routes', 20)])
        for name, rule in self.rules.items():
            with self.subTest(rule=name):
                response = rule['Action']['Block']['CustomResponse']
                self.assertEqual((response['ResponseCode'], response['CustomResponseBodyKey']), (429, 'too-many-requests'))
                self.assertEqual(response['ResponseHeaders'], [{'Name': 'Retry-After', 'Value': '300'}])
                self.assertEqual(rule['VisibilityConfig'], {'SampledRequestsEnabled': True, 'CloudWatchMetricsEnabled': True, 'MetricName': name})
                rate = rule['Statement']['RateBasedStatement']
                self.assertEqual((rate['AggregateKeyType'], rate['EvaluationWindowSec']), ('IP', 300))
                self.assertEqual(set(rule['Statement']), {'RateBasedStatement'})

    def test_the_flood_rule_counts_every_path_at_the_flood_limit(self):
        rate = self.rules['per-address-flood']['Statement']['RateBasedStatement']
        self.assertEqual(rate['Limit'], {'Ref': 'FloodLimit'})
        self.assertNotIn('ScopeDownStatement', rate)

    def test_the_model_rule_is_scoped_to_the_model_calling_routes_the_app_serves(self):
        rate = self.rules['per-address-model-routes']['Statement']['RateBasedStatement']
        self.assertEqual(rate['Limit'], {'Ref': 'ModelRouteLimit'})
        scope = rate['ScopeDownStatement']
        self.assertEqual(list(scope), ['OrStatement'])
        matches = list(byte_matches(scope))
        for depth, _, _, match in matches:
            self.assertLessEqual(depth, 2)  # Or > And > ByteMatch: within WAF's nesting limit.
            self.assertEqual(match['FieldToMatch'], {'UriPath': {}})
            self.assertEqual(match['TextTransformations'], NO_TRANSFORM)
        prefixes = {search for depth, constraint, search, _ in matches if depth == 1 and constraint == 'STARTS_WITH'}
        self.assertEqual(prefixes, self.MODEL_PREFIXES)
        ands = [s['AndStatement']['Statements'] for s in scope['OrStatement']['Statements'] if 'AndStatement' in s]
        self.assertEqual(len(ands), len(self.MODEL_BOX_SUFFIXES))
        for pair in ands:
            self.assertEqual([(m['ByteMatchStatement']['PositionalConstraint'], m['ByteMatchStatement']['SearchString']) for m in pair][0], ('STARTS_WITH', '/v1/boxes/'))
        self.assertEqual({pair[1]['ByteMatchStatement']['SearchString'] for pair in ands}, self.MODEL_BOX_SUFFIXES)
        self.assertEqual({pair[1]['ByteMatchStatement']['PositionalConstraint'] for pair in ands}, {'ENDS_WITH'})

        # Each scoped path is a route constant the app serves, and each one calls the model (or the composer's router model).
        source = {path: (ROOT / 'app' / 'src' / path).read_text() for path in (
            'composer/routes.rs', 'composer/talk.rs', 'mcp/mod.rs', 'actions/mod.rs', 'widgets/mod.rs', 'boxes/routes.rs', 'boxes/ask.rs')}
        self.assertIn('PATH: &str = "/v1/compose"', source['composer/routes.rs'])
        self.assertIn('ASK_PATH: &str = "/v1/ask"', source['composer/talk.rs'])
        self.assertIn('SHAPE_PATH: &str = "/v1/shape"', source['composer/talk.rs'])
        self.assertIn('PATH: &str = "/v1/mcp"', source['mcp/mod.rs'])
        self.assertIn('PATH: &str = "/v1/act"', source['actions/mod.rs'])
        self.assertIn('CUSTOMIZE_PATH: &str = "/v1/widgets/customize"', source['widgets/mod.rs'])
        self.assertIn('EXPLAIN_PATH: &str = "/v1/boxes/explain/{app}/{work}"', source['boxes/routes.rs'])
        self.assertIn('APPS_PATH: &str = "/v1/boxes/{box}/apps"', source['boxes/routes.rs'])
        self.assertIn('BRIEF_PATH: &str = "/v1/boxes/{box}/brief"', source['boxes/routes.rs'])
        self.assertIn('ASK_PATH: &str = "/v1/boxes/{box}/ask"', source['boxes/ask.rs'])
        # Polled and streamed paths, health, assets and Slack's callbacks are not model-limited.
        searched = {search for _, _, search, _ in matches}
        for free in ('/health', '/v1/events', '/v1/shared', '/s/', '/_assets', '/v1/slack', '/v1/work', '/v1/canvas', '/v1/widgets/weather'):
            self.assertFalse(any(s.startswith(free) or free.startswith(s) for s in searched), free)

    def test_the_flood_limit_clears_seven_guest_tabs_and_the_health_check_never_counts(self):
        # The heaviest client is a guest page on /s/<share>: an access poll (AUTHORITY_POLL_MS) and a document poll (setInterval 3000).
        authority = (ROOT / 'app' / 'admin' / 'src' / 'shared' / 'authority.ts').read_text()
        document = (ROOT / 'app' / 'admin' / 'src' / 'shared' / 'document.ts').read_text()
        access_ms = int(re.search(r'AUTHORITY_POLL_MS = (\d+)', authority)[1])
        document_ms = int(re.search(r'window\.setInterval\(\(\) => \{ void refresh\(\); \}, (\d+)\)', document)[1])
        per_tab = 300_000 // access_ms + 300_000 // document_ms
        self.assertEqual(per_tab, 250)
        self.assertLess(7 * per_tab, self.template['Parameters']['FloodLimit']['Default'])
        # A member page: a 30 s access check plus hint-driven refreshes with one retry (APP-195), tens in five minutes.
        work = (ROOT / 'app' / 'admin' / 'src' / 'work' / 'document.ts').read_text()
        self.assertIn('window.setInterval(visible, 30_000)', work)
        # The load balancer's health checks go from the balancer to the targets and never through the web ACL (stack.ts sets them).
        self.assertIn("healthCheck: { path: '/health/ready'", (ROOT / 'infra' / 'src' / 'stack.ts').read_text())

    def test_both_alarms_watch_blocked_requests_of_their_rule_and_publish_to_the_trial_topic(self):
        expected = {'FloodBlockedAlarm': ('flood-blocked', 'per-address-flood'), 'ModelRoutesBlockedAlarm': ('model-routes-blocked', 'per-address-model-routes')}
        for name, (suffix, rule) in expected.items():
            with self.subTest(alarm=name):
                alarm = self.resources[name]['Properties']
                self.assertEqual(alarm['AlarmName'], {'Fn::Sub': '${AWS::StackName}-' + suffix})
                self.assertEqual((alarm['Namespace'], alarm['MetricName'], alarm['Statistic'], alarm['Period'], alarm['EvaluationPeriods'],
                                  alarm['Threshold'], alarm['ComparisonOperator'], alarm['TreatMissingData']),
                                 ('AWS/WAFV2', 'BlockedRequests', 'Sum', 300, 1, 1, 'GreaterThanOrEqualToThreshold', 'notBreaching'))
                self.assertEqual(alarm['Dimensions'], [{'Name': 'WebACL', 'Value': self.acl['Name']}, {'Name': 'Region', 'Value': {'Ref': 'AWS::Region'}},
                                                       {'Name': 'Rule', 'Value': rule}])
                self.assertIn(rule, self.rules)
                self.assertEqual((alarm['AlarmActions'], alarm['OKActions']), ([{'Ref': 'AlarmTopicArn'}], [{'Ref': 'AlarmTopicArn'}]))
                self.assertGreater(len(alarm['AlarmDescription']), 60)

    def test_no_private_identifier_is_embedded(self):
        self.assert_no_private_identifier(WAF, self.template)


class DeploymentContractTests(unittest.TestCase):
    def test_infra_src_owns_none_of_these_resources_and_the_templates_import_nothing_from_it(self):
        # Both templates stand beside GenuiSyntheticFoundation: the CDK stack has no WAF, trail, analyzer or EventBridge rule
        # to conflict with, and the templates reach the beta only through parameters (no ImportValue, no stack name).
        for source in (ROOT / 'infra' / 'src').glob('*.ts'):
            text = source.read_text()
            for owned in ('aws-wafv2', 'AWS::WAFv2', 'aws-cloudtrail', 'AWS::CloudTrail', 'aws-accessanalyzer', 'AWS::AccessAnalyzer',
                          'aws-events', 'AWS::Events::Rule'):
                self.assertFalse(owned in text, f'{source.name}: {owned}')
        for template in (EVENTS, WAF):
            self.assertNotIn('Fn::ImportValue', template.read_text())
        self.assertEqual({'Ref': 'LoadBalancerArn'}, json.loads(WAF.read_text())['Resources']['WebAclAssociation']['Properties']['ResourceArn'])


if __name__ == '__main__':
    unittest.main()
