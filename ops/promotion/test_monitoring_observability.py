"""Pins the standalone observability template (ops/monitoring/observability.json): the two uptime probes and their alarms,
the two SLO alarms, the five metric filters on the app log's named events and their alarms (APP-219), the golden-signals
dashboard, the resource set, and that it touches nothing of the beta's stack and hard-codes no log group or account."""
import json
from pathlib import Path
import re
import unittest

MONITORING = Path(__file__).resolve().parents[1] / 'monitoring'
ROOT = MONITORING.parents[1]
TEMPLATE = MONITORING / 'observability.json'
RUNBOOK = ROOT / 'docs' / 'operations' / 'TRIAL-RUNBOOK.md'
STACK_NAME = 'forma-beta-observability'
TOPIC = {'Ref': 'AlarmTopicArn'}
LOAD_BALANCER = [{'Name': 'LoadBalancer', 'Value': {'Ref': 'LoadBalancerFullName'}}]
PROBE_REGIONS = ['us-east-1', 'us-west-2', 'eu-west-1']
RUNBOOK_SECTION = 'docs/operations/TRIAL-RUNBOOK.md §11.8'
LOG_GROUP = {'Ref': 'AppLogGroupName'}
EVENT_NAMESPACE = 'Forma/Beta'
# The metric filters (APP-219): resource -> (filter name suffix, the exact pattern, the metric it counts). The events and their
# fields are the service's (app/src/composer/model.rs, app/src/http/mod.rs, app/src/imports/sandbox.rs); percent is a number.
FILTERS = {
    'ModelUsageWarningFilter': ('model-usage-warning', '{ $.event = "model.usage_threshold" && $.percent = 80 }', 'ModelUsageWarning'),
    'ModelUsageReachedFilter': ('model-usage-reached', '{ $.event = "model.usage_threshold" && $.percent = 100 }', 'ModelUsageReached'),
    'ModelCallFailedFilter': ('model-call-failed', '{ $.event = "model.call_failed" }', 'ModelCallFailed'),
    'ServerErrorFilter': ('server-error', '{ $.event = "http.server_error" && $.route != "/health/ready" }', 'ServerError'),
    'ParserKilledFilter': ('parser-killed', '{ $.event = "parser.killed" }', 'ParserKilled'),
}
# The alarms on them: resource -> (alarm name suffix, metric, threshold, operator, evaluation periods, datapoints to alarm).
EVENT_ALARMS = {
    'AiUsageWarningAlarm': ('ai-usage-80', 'ModelUsageWarning', 1, 'GreaterThanOrEqualToThreshold', 1, 1),
    'AiUsageReachedAlarm': ('ai-usage-100', 'ModelUsageReached', 1, 'GreaterThanOrEqualToThreshold', 1, 1),
    'ModelFailuresAlarm': ('model-failures', 'ModelCallFailed', 5, 'GreaterThanThreshold', 1, 1),
    'ServerErrorsAlarm': ('server-errors', 'ServerError', 5, 'GreaterThanThreshold', 3, 2),
    'ParserKillsAlarm': ('parser-kills', 'ParserKilled', 3, 'GreaterThanThreshold', 1, 1),
}
# Sample values for the dashboard's Fn::Sub: parameters, the two health checks' ids (their Ref) and the region.
SAMPLES = {
    'AlarmTopicArn': 'arn:aws:sns:us-east-1:123456789012:forma-beta-trial-alarms-AlarmTopic-ABCDEF123456',
    'LoadBalancerFullName': 'app/Genui-LoadB-ABCDEF123456/0123456789abcdef',
    'TargetGroupFullName': 'targetgroup/Genui-Targe-ABCDEF123456/0123456789abcdef',
    'ClusterName': 'GenuiSyntheticFoundation-Cluster-ABCDEF',
    'ServiceName': 'GenuiSyntheticFoundation-Service-ABCDEF',
    'DBInstanceIdentifier': 'genuisyntheticfoundation-database-abcdef',
    'FauthInstanceId': 'i-0123456789abcdef0',
    'WebAclName': 'forma-beta-waf-web-acl',
    'AppHostname': 'beta.getforma.dev',
    'FauthHostname': 'auth-staging.getforma.dev',
    'AppLogGroupName': 'GenuiSyntheticFoundation-LogsABCDEF12-abcdefABCDEF',
    'AppHealthCheck': '11111111-2222-3333-4444-555555555555',
    'FauthHealthCheck': '66666666-7777-8888-9999-000000000000',
    'AWS::Region': 'us-east-1',
}


def strings(value):
    if isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)
    elif isinstance(value, str):
        yield value


def github_anchor(heading):
    """GitHub's heading slug: lowercase, punctuation dropped, spaces to hyphens."""
    return re.sub(r'[^a-z0-9 _-]', '', heading.strip().lower()).replace(' ', '-')


def metric_rows(widget):
    """(namespace, metric, dimensions, options) per metric row, ('expression', text, {}, options) per expression row."""
    for row in widget['properties']['metrics']:
        if isinstance(row[0], dict):
            yield ('expression', row[0]['expression'], {}, row[0])
            continue
        options = row[-1] if isinstance(row[-1], dict) else None
        names = row[:-1] if options is not None else row
        dimensions = dict(zip(names[2::2], names[3::2]))
        yield (names[0], names[1], dimensions, options or {})


class ObservabilityTemplateTests(unittest.TestCase):
    def setUp(self):
        self.template = json.loads(TEMPLATE.read_text())
        self.parameters = self.template['Parameters']
        self.resources = self.template['Resources']
        self.alarms = {name: r['Properties'] for name, r in self.resources.items() if r['Type'] == 'AWS::CloudWatch::Alarm'}

    def dashboard(self):
        body = self.resources['GoldenSignalsDashboard']['Properties']['DashboardBody']['Fn::Sub']
        self.assertIsInstance(body, str)
        placeholders = set(re.findall(r'\$\{([^}]+)\}', body))
        # Every placeholder is a parameter, a resource of this template or the region, so Fn::Sub resolves them all.
        self.assertLessEqual(placeholders, set(self.parameters) | set(self.resources) | {'AWS::Region'})
        substituted = re.sub(r'\$\{([^}]+)\}', lambda m: SAMPLES[m.group(1)], body)
        self.assertNotIn('${', substituted)
        return json.loads(substituted)

    def test_parameters_are_the_eleven_identifiers_with_the_patterns_of_the_alarm_and_waf_stacks(self):
        self.assertEqual(self.template['AWSTemplateFormatVersion'], '2010-09-09')
        self.assertEqual(set(self.parameters), {'AlarmTopicArn', 'LoadBalancerFullName', 'TargetGroupFullName', 'ClusterName', 'ServiceName',
                                                'DBInstanceIdentifier', 'FauthInstanceId', 'WebAclName', 'AppHostname', 'FauthHostname',
                                                'AppLogGroupName'})
        for name, parameter in self.parameters.items():
            with self.subTest(parameter=name):
                self.assertEqual(parameter['Type'], 'String')
                self.assertIn('AllowedPattern', parameter)
                self.assertTrue(re.fullmatch(parameter['AllowedPattern'], SAMPLES[name]), SAMPLES[name])
        # Only the hostnames have defaults; every identifier is derived and passed (runbook 11.8).
        self.assertEqual({name: p['Default'] for name, p in self.parameters.items() if 'Default' in p},
                         {'AppHostname': 'beta.getforma.dev', 'FauthHostname': 'auth-staging.getforma.dev'})
        # The shared identifiers carry exactly the patterns the alarms and WAF stacks already validate them with.
        alarms = json.loads((MONITORING / 'alarms.json').read_text())['Parameters']
        waf = json.loads((MONITORING / 'waf.json').read_text())['Parameters']
        for name in ('LoadBalancerFullName', 'TargetGroupFullName', 'ClusterName', 'ServiceName', 'DBInstanceIdentifier', 'FauthInstanceId'):
            self.assertEqual(self.parameters[name]['AllowedPattern'], alarms[name]['AllowedPattern'], name)
        self.assertEqual(self.parameters['AlarmTopicArn']['AllowedPattern'], waf['AlarmTopicArn']['AllowedPattern'])
        self.assertFalse(re.fullmatch(self.parameters['AlarmTopicArn']['AllowedPattern'], 'forma-beta-trial-alarms'))
        self.assertFalse(re.fullmatch(self.parameters['WebAclName']['AllowedPattern'], 'arn:aws:wafv2:us-east-1:123456789012:regional/webacl/x/y'))
        # The hostnames: strict lowercase DNS names, no scheme, port, path, IP address or trailing dot.
        for name in ('AppHostname', 'FauthHostname'):
            pattern = self.parameters[name]['AllowedPattern']
            self.assertEqual(self.parameters[name]['MaxLength'], 253)
            for good in ('beta.getforma.dev', 'auth-staging.getforma.dev', 'a.b.example.test'):
                self.assertTrue(re.fullmatch(pattern, good), good)
            for bad in ('Beta.getforma.dev', 'https://beta.getforma.dev', 'beta.getforma.dev/', 'beta.getforma.dev:443', 'beta.getforma.dev.',
                        '-beta.getforma.dev', 'beta-.getforma.dev', 'beta..getforma.dev', 'localhost', '10.0.0.1', 'beta getforma.dev', ''):
                self.assertFalse(re.fullmatch(pattern, bad), bad)
        # The app log group: CloudWatch Logs' own name alphabet, a name and never an ARN, derived at deploy time (runbook 11's LOG_GROUP).
        log_group = self.parameters['AppLogGroupName']
        self.assertEqual(log_group['AllowedPattern'], '^[A-Za-z0-9_/.#-]{1,512}$')
        self.assertNotIn('Default', log_group)
        for good in (SAMPLES['AppLogGroupName'], '/ecs/forma-beta', 'a', 'x' * 512):
            self.assertTrue(re.fullmatch(log_group['AllowedPattern'], good), good)
        for bad in ('', 'x' * 513, 'arn:aws:logs:us-east-1:123456789012:log-group:/ecs/forma-beta', 'a log group', 'group:*', 'génui'):
            self.assertFalse(re.fullmatch(log_group['AllowedPattern'], bad), bad)

    def test_resources_are_two_health_checks_five_metric_filters_nine_alarms_and_one_dashboard(self):
        types = {name: r['Type'] for name, r in self.resources.items()}
        self.assertEqual(types, {'AppHealthCheck': 'AWS::Route53::HealthCheck', 'FauthHealthCheck': 'AWS::Route53::HealthCheck',
                                 'AppUnreachableAlarm': 'AWS::CloudWatch::Alarm', 'FauthUnreachableAlarm': 'AWS::CloudWatch::Alarm',
                                 'ErrorRateAlarm': 'AWS::CloudWatch::Alarm', 'SlowMedianAlarm': 'AWS::CloudWatch::Alarm',
                                 'ModelUsageWarningFilter': 'AWS::Logs::MetricFilter', 'ModelUsageReachedFilter': 'AWS::Logs::MetricFilter',
                                 'ModelCallFailedFilter': 'AWS::Logs::MetricFilter', 'ServerErrorFilter': 'AWS::Logs::MetricFilter',
                                 'ParserKilledFilter': 'AWS::Logs::MetricFilter',
                                 'AiUsageWarningAlarm': 'AWS::CloudWatch::Alarm', 'AiUsageReachedAlarm': 'AWS::CloudWatch::Alarm',
                                 'ModelFailuresAlarm': 'AWS::CloudWatch::Alarm', 'ServerErrorsAlarm': 'AWS::CloudWatch::Alarm',
                                 'ParserKillsAlarm': 'AWS::CloudWatch::Alarm',
                                 'GoldenSignalsDashboard': 'AWS::CloudWatch::Dashboard'})
        # Standalone and free of the paid or privileged services: no IAM, no Synthetics canaries, nothing of the beta. Of CloudWatch Logs
        # only metric filters on the app's existing log group (a custom metric each, $0.30): no log group of its own, no subscription
        # filter or destination (a Lambda or Kinesis consumer), no saved Logs Insights query.
        text = TEMPLATE.read_text()
        for forbidden in ('AWS::IAM::', 'AWS::Synthetics::', 'AWS::RDS::', 'AWS::ElasticLoadBalancingV2::', 'AWS::ECS::', 'AWS::EC2::',
                          'AWS::WAFv2::', 'AWS::SNS::', 'AWS::Lambda::', 'AWS::Kinesis'):
            self.assertFalse(any(t.startswith(forbidden) for t in types.values()), forbidden)
        self.assertEqual({t for t in types.values() if t.startswith('AWS::Logs::')}, {'AWS::Logs::MetricFilter'})
        for forbidden in ('AWS::IAM', 'AWS::Synthetics', 'aws-synthetics', 'Fn::ImportValue', 'AWS::Logs::LogGroup', 'AWS::Logs::SubscriptionFilter',
                          'AWS::Logs::Destination', 'AWS::Logs::QueryDefinition', 'AWS::Logs::Delivery'):
            self.assertNotIn(forbidden, text)
        self.assertEqual(self.template['Outputs']['AppHealthCheckId']['Value'], {'Ref': 'AppHealthCheck'})
        self.assertEqual(self.template['Outputs']['FauthHealthCheckId']['Value'], {'Ref': 'FauthHealthCheck'})
        self.assertIn('${GoldenSignalsDashboard}', self.template['Outputs']['DashboardUrl']['Value']['Fn::Sub'])

    def test_each_health_check_probes_https_from_exactly_three_regions_with_no_paid_optional_feature(self):
        expected = {'AppHealthCheck': ('AppHostname', '/health/live', 'app'), 'FauthHealthCheck': ('FauthHostname', '/health', 'fauth')}
        for name, (hostname, path, tag) in expected.items():
            with self.subTest(health_check=name):
                properties = self.resources[name]['Properties']
                # Exact equality: any extra key (SearchString, MeasureLatency, Inverted, an IP address, child checks) fails here.
                self.assertEqual(properties['HealthCheckConfig'], {
                    'Type': 'HTTPS', 'FullyQualifiedDomainName': {'Ref': hostname}, 'Port': 443, 'ResourcePath': path, 'EnableSNI': True,
                    'RequestInterval': 30, 'FailureThreshold': 3, 'Regions': PROBE_REGIONS})
                self.assertEqual(properties['HealthCheckTags'], [{'Key': 'Name', 'Value': {'Fn::Sub': '${AWS::StackName}-' + tag}}])
                self.assertEqual(set(properties), {'HealthCheckConfig', 'HealthCheckTags'})
                config = properties['HealthCheckConfig']
                # Route 53 needs at least three regions; these are three distinct valid checker regions.
                self.assertEqual(len(set(config['Regions'])), 3)
                self.assertLessEqual(set(config['Regions']), {'us-east-1', 'us-west-1', 'us-west-2', 'eu-west-1', 'ap-southeast-1', 'ap-southeast-2',
                                                              'ap-northeast-1', 'sa-east-1'})
                # The paid optional features, each $1 a month: string matching, the 10 s fast interval, latency measurement.
                self.assertNotIn('STR_MATCH', config['Type'])
                self.assertNotIn('SearchString', config)
                self.assertNotEqual(config['RequestInterval'], 10)
                self.assertNotIn('MeasureLatency', config)
        text = TEMPLATE.read_text()
        for paid in ('HTTPS_STR_MATCH', 'HTTP_STR_MATCH', 'SearchString', 'MeasureLatency'):
            self.assertNotIn(paid, text)
        # The paths the probes ask are the ones the app and fAuth serve (the app's route; fAuth answered 200 at /health on 2026-11-15).
        # /health/live through the balancer: the balancer answers 503 itself when no target is healthy, so an outage
        # still fails the probe, while readiness's 4-minute credential-rotation blip (APP-233) no longer counts as 5xx.
        self.assertIn('"/health/live"', (ROOT / 'app' / 'src' / 'main.rs').read_text())

    def test_the_unreachable_alarms_fire_after_three_failed_minutes_and_keep_state_on_gaps(self):
        expected = {'AppUnreachableAlarm': ('AppHealthCheck', 'app-unreachable'), 'FauthUnreachableAlarm': ('FauthHealthCheck', 'fauth-unreachable')}
        for name, (health_check, suffix) in expected.items():
            with self.subTest(alarm=name):
                alarm = self.alarms[name]
                self.assertEqual((alarm['Namespace'], alarm['MetricName'], alarm['Statistic'], alarm['Period'], alarm['EvaluationPeriods'],
                                  alarm['DatapointsToAlarm'], alarm['Threshold'], alarm['ComparisonOperator'], alarm['TreatMissingData']),
                                 ('AWS/Route53', 'HealthCheckStatus', 'Minimum', 60, 3, 3, 1, 'LessThanThreshold', 'missing'))
            # A real outage still publishes status 0; a gap is metric delivery (seen 2026-11-15 at creation), so it keeps the last state.
                self.assertEqual(alarm['Dimensions'], [{'Name': 'HealthCheckId', 'Value': {'Ref': health_check}}])
                self.assertEqual(alarm['AlarmName'], {'Fn::Sub': '${AWS::StackName}-' + suffix})

    def test_the_error_rate_alarm_divides_both_5xx_counts_by_requests_only_from_100_requests(self):
        alarm = self.alarms['ErrorRateAlarm']
        for single in ('Namespace', 'MetricName', 'Statistic', 'ExtendedStatistic', 'Period', 'Dimensions'):
            self.assertNotIn(single, alarm)
        metrics = {m['Id']: m for m in alarm['Metrics']}
        self.assertEqual(list(metrics), ['target5xx', 'elb5xx', 'requests', 'errors', 'rate'])
        for metric_id, metric_name in (('target5xx', 'HTTPCode_Target_5XX_Count'), ('elb5xx', 'HTTPCode_ELB_5XX_Count'), ('requests', 'RequestCount')):
            self.assertEqual(metrics[metric_id], {'Id': metric_id, 'ReturnData': False, 'MetricStat': {
                'Metric': {'Namespace': 'AWS/ApplicationELB', 'MetricName': metric_name, 'Dimensions': LOAD_BALANCER}, 'Period': 300, 'Stat': 'Sum'}})
        # The 5xx counts are published only when non-zero: FILL makes a quiet period 0 rather than a gap that drops the sum.
        self.assertEqual(metrics['errors']['Expression'], 'FILL(target5xx, 0) + FILL(elb5xx, 0)')
        self.assertEqual(metrics['rate']['Expression'], 'IF(requests >= 100, errors/requests*100, 0)')
        self.assertEqual([m['Id'] for m in alarm['Metrics'] if m['ReturnData']], ['rate'])
        self.assertEqual((alarm['EvaluationPeriods'], alarm['DatapointsToAlarm'], alarm['Threshold'], alarm['ComparisonOperator'], alarm['TreatMissingData']),
                         (3, 2, 2, 'GreaterThanThreshold', 'notBreaching'))
        self.assertEqual(alarm['AlarmName'], {'Fn::Sub': '${AWS::StackName}-error-rate'})

        # The expression, evaluated the way CloudWatch does for one period: gated on 100 requests, a percentage above 2 breaches.
        def rate(target5xx, elb5xx, requests):
            errors = (target5xx or 0) + (elb5xx or 0)
            return errors / requests * 100 if requests >= 100 else 0
        self.assertEqual(rate(3, None, 99), 0)          # 3% of 99 requests: below the gate (the probes alone send about 55), never alarms.
        self.assertGreater(rate(2, 1, 100), 2)          # 3% of 100.
        self.assertEqual(rate(1, 1, 100), 2)            # exactly 2% does not breach GreaterThanThreshold.
        self.assertLessEqual(rate(None, None, 5000), 2)

    def test_the_slow_median_alarm_watches_p50_because_model_calls_make_the_tail_slow(self):
        alarm = self.alarms['SlowMedianAlarm']
        self.assertEqual((alarm['Namespace'], alarm['MetricName'], alarm['ExtendedStatistic'], alarm['Period'], alarm['EvaluationPeriods'],
                          alarm['DatapointsToAlarm'], alarm['Threshold'], alarm['ComparisonOperator'], alarm['TreatMissingData']),
                         ('AWS/ApplicationELB', 'TargetResponseTime', 'p50', 300, 3, 3, 2, 'GreaterThanThreshold', 'notBreaching'))
        self.assertNotIn('Statistic', alarm)
        self.assertEqual(alarm['Dimensions'], LOAD_BALANCER)
        self.assertEqual(alarm['AlarmName'], {'Fn::Sub': '${AWS::StackName}-slow-median'})
        self.assertIn('p95', alarm['AlarmDescription'])
        self.assertIn('tens of seconds', alarm['AlarmDescription'])

    def test_each_metric_filter_counts_one_named_event_of_the_app_log_group_passed_by_parameter(self):
        filters = {name: r for name, r in self.resources.items() if r['Type'] == 'AWS::Logs::MetricFilter'}
        self.assertEqual(set(filters), set(FILTERS))
        for name, (suffix, pattern, metric) in FILTERS.items():
            with self.subTest(filter=name):
                # Exact equality: the log group is the parameter, never a name; the pattern is the one named event (and, for the usage
                # thresholds, the numeric percent); one count of 1 per matching line, 0 for a period whose lines all miss (so the
                # metric exists and the alarm evaluates); no dimension (DefaultValue is refused with one) and no unit.
                self.assertEqual(filters[name]['Properties'], {
                    'FilterName': {'Fn::Sub': '${AWS::StackName}-' + suffix},
                    'LogGroupName': LOG_GROUP,
                    'FilterPattern': pattern,
                    'MetricTransformations': [{'MetricNamespace': EVENT_NAMESPACE, 'MetricName': metric, 'MetricValue': '1', 'DefaultValue': 0}]})
                self.assertEqual(set(filters[name]), {'Type', 'Properties'})
        self.assertEqual(len({metric for _, _, metric in FILTERS.values()}), len(FILTERS), 'one metric per filter')
        self.assertEqual(len({pattern for _, pattern, _ in FILTERS.values()}), len(FILTERS), 'one pattern per filter')

    def test_the_filter_patterns_name_the_events_the_service_logs_and_compare_percent_as_a_number(self):
        # The service writes one flat JSON object per line (tracing_subscriber's json layer with flatten_event, so `event` and the
        # event's fields are top-level keys); each pattern is the JSON metric-filter syntax on that `event` key.
        logs = (ROOT / 'app' / 'src' / 'config.rs').read_text()
        self.assertIn('.json()', logs)
        self.assertIn('.flatten_event(true)', logs)
        sources = {'model.usage_threshold': 'composer/model.rs', 'model.call_failed': 'composer/model.rs',
                   'http.server_error': 'http/mod.rs', 'parser.killed': 'imports/sandbox.rs'}
        for event, source in sources.items():
            self.assertIn(f'event = "{event}"', (ROOT / 'app' / 'src' / source).read_text(), event)
        patterns = [r['Properties']['FilterPattern'] for r in self.resources.values() if r['Type'] == 'AWS::Logs::MetricFilter']
        self.assertEqual(len(patterns), 5)
        for pattern in patterns:
            # The one exclusion: readiness's 503 is a state the balancer acts on (-unhealthy-targets), not a fault (APP-233).
            self.assertRegex(pattern, r'^\{ \$\.event = "[a-z_.]+"( && \$\.[a-z_]+ = [0-9]+| && \$\.route != "/health/ready")? \}$')
        self.assertEqual({re.search(r'\$\.event = "([^"]+)"', p).group(1) for p in patterns}, set(sources))
        # `percent` is a u32 (a JSON number), so the usage patterns compare a number, never a quoted string, and they name exactly
        # the service's thresholds (USAGE_THRESHOLDS: once per UTC day per scope at each).
        model = (ROOT / 'app' / 'src' / 'composer' / 'model.rs').read_text()
        self.assertIn('pub percent: u32', model)
        thresholds = re.search(r'pub const USAGE_THRESHOLDS: \[u32; 2\] = \[(\d+), (\d+)\];', model)
        self.assertIsNotNone(thresholds)
        usage = {int(re.search(r'\$\.percent = (\d+)', p).group(1)) for p in patterns if 'model.usage_threshold' in p}
        self.assertEqual(usage, {int(thresholds.group(1)), int(thresholds.group(2))})
        for pattern in patterns:
            self.assertNotRegex(pattern, r'\$\.percent = "')
        # The 80 % description's words are the ones asked for, and the cap it names is the service's own setting.
        self.assertIn('pub const DAILY_CALLS_ENV: &str = "GENUI_MODEL_DAILY_CALLS";', model)
        self.assertIn("Today's AI allowance for the organization or a member reached 80 %; building and Ask pause at 100 % until 00:00 UTC. "
                      'Check /admin and the canvas; raise GENUI_MODEL_DAILY_CALLS only with a release.',
                      self.alarms['AiUsageWarningAlarm']['AlarmDescription'])
        self.assertIn('00:00 UTC', self.alarms['AiUsageReachedAlarm']['AlarmDescription'])

    def test_the_event_alarms_sum_five_minutes_and_stay_quiet_when_nothing_is_logged(self):
        metrics = {metric for _, _, metric in FILTERS.values()}
        for name, (suffix, metric, threshold, operator, evaluation_periods, datapoints) in EVENT_ALARMS.items():
            with self.subTest(alarm=name):
                alarm = self.alarms[name]
                self.assertIn(metric, metrics, 'the alarm watches a metric a filter publishes')
                self.assertEqual((alarm['Namespace'], alarm['MetricName'], alarm['Statistic'], alarm['Period'], alarm['EvaluationPeriods'],
                                  alarm['DatapointsToAlarm'], alarm['Threshold'], alarm['ComparisonOperator'], alarm['TreatMissingData']),
                                 (EVENT_NAMESPACE, metric, 'Sum', 300, evaluation_periods, datapoints, threshold, operator, 'notBreaching'))
                # No events in a period means nothing happened, not that the alarm cannot tell; the filters publish no dimension.
                for absent in ('Dimensions', 'ExtendedStatistic', 'Metrics', 'Unit'):
                    self.assertNotIn(absent, alarm)
                self.assertEqual((alarm['AlarmActions'], alarm['OKActions']), ([TOPIC], [TOPIC]))
                self.assertEqual(alarm['AlarmName'], {'Fn::Sub': '${AWS::StackName}-' + suffix})
        # "At least one" for the once-a-day thresholds, "more than N" for the counts.
        self.assertEqual({a['Threshold'] for n, a in self.alarms.items() if n in EVENT_ALARMS and a['ComparisonOperator'] == 'GreaterThanOrEqualToThreshold'}, {1})
        self.assertEqual({(a['EvaluationPeriods'], a['DatapointsToAlarm']) for n, a in self.alarms.items() if n in EVENT_ALARMS}, {(1, 1), (3, 2)})
        self.assertEqual({n for n, a in self.alarms.items() if n in EVENT_ALARMS and a['EvaluationPeriods'] == 3}, {'ServerErrorsAlarm'})

    def test_every_alarm_notifies_the_trial_topic_and_names_the_runbook_section(self):
        self.assertEqual(set(self.alarms), {'AppUnreachableAlarm', 'FauthUnreachableAlarm', 'ErrorRateAlarm', 'SlowMedianAlarm', *EVENT_ALARMS})
        names = set()
        for name, alarm in self.alarms.items():
            with self.subTest(alarm=name):
                self.assertEqual((alarm['AlarmActions'], alarm['OKActions']), ([TOPIC], [TOPIC]))
                self.assertNotIn('InsufficientDataActions', alarm)
                self.assertIn(RUNBOOK_SECTION, alarm['AlarmDescription'])
                self.assertGreater(len(alarm['AlarmDescription']), 60)
                self.assertLessEqual(len(alarm['AlarmDescription']), 1024)
                names.add(alarm['AlarmName']['Fn::Sub'].replace('${AWS::StackName}', STACK_NAME))
        self.assertEqual(names, {'forma-beta-observability-app-unreachable', 'forma-beta-observability-fauth-unreachable',
                                 'forma-beta-observability-error-rate', 'forma-beta-observability-slow-median',
                                 'forma-beta-observability-ai-usage-80', 'forma-beta-observability-ai-usage-100',
                                 'forma-beta-observability-model-failures', 'forma-beta-observability-server-errors',
                                 'forma-beta-observability-parser-kills'})
        headings = [line for line in RUNBOOK.read_text().splitlines() if line.startswith('### 11.8 ')]
        self.assertEqual(len(headings), 1)

    def test_the_dashboard_body_parses_after_substitution_and_names_every_golden_signal(self):
        dashboard = self.resources['GoldenSignalsDashboard']['Properties']
        self.assertEqual(dashboard['DashboardName'], 'forma-beta-golden-signals')
        body = self.dashboard()
        widgets = sorted(body['widgets'], key=lambda w: (w['y'], w['x']))
        # Only text and metric widgets: no Logs Insights widget (it bills per GB scanned) and no canary.
        self.assertLessEqual({w['type'] for w in widgets}, {'text', 'metric'})
        for widget in widgets:
            self.assertLessEqual(widget['x'] + widget['width'], 24)
        metric_widgets = [w for w in widgets if w['type'] == 'metric']
        for widget in metric_widgets:
            self.assertEqual(widget['properties']['region'], 'us-east-1', widget['properties']['title'])
            for row in widget['properties']['metrics']:
                self.assertNotIn('.', row, 'every row names its metric in full')
        rows = []
        for widget in metric_widgets:
            row = widget['properties']['title'].split(':')[0]
            if row not in rows:
                rows.append(row)
        self.assertEqual(rows, ['Traffic', 'Errors', 'Latency', 'Saturation', 'Health', 'Email', 'Events'])

        found = {(ns, name, tuple(sorted(dims.items())), options.get('stat', '')) for w in metric_widgets for ns, name, dims, options in metric_rows(w)}
        alb = ('LoadBalancer', SAMPLES['LoadBalancerFullName'])
        waf = (('Region', 'us-east-1'), ('Rule', 'ALL'), ('WebACL', SAMPLES['WebAclName']))
        ecs = (('ClusterName', SAMPLES['ClusterName']), ('ServiceName', SAMPLES['ServiceName']))
        rds = (('DBInstanceIdentifier', SAMPLES['DBInstanceIdentifier']),)
        expected = {
            # Traffic
            ('AWS/ApplicationELB', 'RequestCount', (alb,)),
            ('AWS/WAFV2', 'AllowedRequests', waf), ('AWS/WAFV2', 'BlockedRequests', waf),
            # Errors
            ('AWS/ApplicationELB', 'HTTPCode_Target_4XX_Count', (alb,)), ('AWS/ApplicationELB', 'HTTPCode_Target_5XX_Count', (alb,)),
            ('AWS/ApplicationELB', 'HTTPCode_ELB_4XX_Count', (alb,)), ('AWS/ApplicationELB', 'HTTPCode_ELB_5XX_Count', (alb,)),
            # Latency
            ('AWS/ApplicationELB', 'TargetResponseTime', (alb,)),
            # Saturation
            ('AWS/ECS', 'CPUUtilization', ecs), ('AWS/ECS', 'MemoryUtilization', ecs),
            ('AWS/RDS', 'CPUUtilization', rds), ('AWS/RDS', 'DatabaseConnections', rds), ('AWS/RDS', 'FreeableMemory', rds),
            ('AWS/EC2', 'CPUUtilization', (('InstanceId', SAMPLES['FauthInstanceId']),)),
            # Health
            ('AWS/Route53', 'HealthCheckStatus', (('HealthCheckId', SAMPLES['AppHealthCheck']),)),
            ('AWS/Route53', 'HealthCheckStatus', (('HealthCheckId', SAMPLES['FauthHealthCheck']),)),
            ('AWS/ApplicationELB', 'HealthyHostCount', (alb, ('TargetGroup', SAMPLES['TargetGroupFullName']))),
            # Email (account-wide, no dimension)
            ('AWS/SES', 'Send', ()), ('AWS/SES', 'Bounce', ()), ('AWS/SES', 'Complaint', ()),
            # Events: the five metrics the filters publish, no dimension
            *((EVENT_NAMESPACE, metric, ()) for _, _, metric in FILTERS.values()),
        }
        self.assertEqual({(ns, name, dims) for ns, name, dims, _ in found if ns != 'expression'}, expected)
        events_row = [w for w in metric_widgets if w['properties']['title'].startswith('Events: ')]
        self.assertEqual({name for w in events_row for ns, name, *_ in metric_rows(w)}, {metric for _, _, metric in FILTERS.values()})
        for widget in events_row:
            self.assertEqual((widget['properties']['stat'], widget['properties']['period']), ('Sum', 300))
        latency = {stat for ns, name, _, stat in found if name == 'TargetResponseTime'}
        self.assertEqual(latency, {'p50', 'p90', 'p99'})
        # The error-rate expression is the alarm's, over the same three metrics.
        expressions = {name for ns, name, _, _ in found if ns == 'expression'}
        alarm_metrics = {m['Id']: m for m in self.alarms['ErrorRateAlarm']['Metrics']}
        self.assertEqual(expressions, {alarm_metrics['errors']['Expression'], alarm_metrics['rate']['Expression']})
        rate_widget = next(w for w in metric_widgets if any(ns == 'expression' for ns, *_ in metric_rows(w)))
        ids = {options.get('id'): name for ns, name, _, options in metric_rows(rate_widget)}
        self.assertEqual({k: v for k, v in ids.items() if k in ('target5xx', 'elb5xx', 'requests')},
                         {'target5xx': 'HTTPCode_Target_5XX_Count', 'elb5xx': 'HTTPCode_ELB_5XX_Count', 'requests': 'RequestCount'})
        # Free: one of the account's first three dashboards, at most 50 metrics.
        self.assertLessEqual(sum(1 for w in metric_widgets for ns, *_ in metric_rows(w) if ns != 'expression'), 50)

    def test_the_dashboard_opens_with_one_line_per_row_and_a_working_runbook_link(self):
        widgets = sorted(self.dashboard()['widgets'], key=lambda w: (w['y'], w['x']))
        text = widgets[0]
        self.assertEqual((text['type'], text['x'], text['y'], text['width']), ('text', 0, 0, 24))
        self.assertEqual(sum(w['type'] == 'text' for w in widgets), 1)
        lines = text['properties']['markdown'].splitlines()
        for row in ('Traffic', 'Errors', 'Latency', 'Saturation', 'Health', 'Email', 'Events'):
            self.assertEqual(sum(line.startswith(f'* **{row}**: ') for line in lines), 1, row)
        heading = next(line for line in RUNBOOK.read_text().splitlines() if line.startswith('### 11.8 '))
        link = 'https://github.com/getforma-dev/forma-app-platform/blob/main/docs/operations/TRIAL-RUNBOOK.md#' + github_anchor(heading.removeprefix('### '))
        self.assertIn(f']({link})', text['properties']['markdown'])

    def test_validate_template_would_pass_and_report_no_capability(self):
        # validate-template reports a capability only for IAM resources, macros/transforms and nested stacks; there are none
        # (`aws cloudformation validate-template` printed no Capabilities on 2026-11-15, runbook 11.8). It refuses a
        # template description over 1024 characters.
        self.assertLessEqual(len(self.template['Description']), 1024)
        self.assertNotIn('Transform', self.template)
        for resource in self.resources.values():
            self.assertFalse(resource['Type'].startswith(('AWS::IAM::', 'AWS::Serverless::', 'AWS::CloudFormation::')), resource['Type'])

    def test_no_private_identifier_is_embedded(self):
        text = TEMPLATE.read_text()
        for private in ('877573132739', 'dkr.ecr', '/Users/', 'amazonaws.com/', 'Fn::ImportValue', 'AWS::AccountId',
                        # The app log group's generated name (GenuiSyntheticFoundation-Logs<hash>-<suffix>) is passed as AppLogGroupName.
                        'GenuiSyntheticFoundation-Logs', 'Logs6819BB44'):
            self.assertFalse(private in text, private)
        self.assertIsNone(re.search(r'(?<![0-9])[0-9]{12}(?![0-9])', text))
        self.assertEqual([r['Properties']['LogGroupName'] for r in self.resources.values() if r['Type'] == 'AWS::Logs::MetricFilter'], [LOG_GROUP] * 5)
        patterns = {p['AllowedPattern'] for p in self.parameters.values()}
        for value in strings(self.template):
            if value in patterns:
                continue
            self.assertNotIn('arn:aws:', value, value)
            self.assertNotRegex(value, r'^subnet-|^sg-|^vpc-|^i-')

    def test_infra_src_owns_none_of_these_resources(self):
        # The template stands beside GenuiSyntheticFoundation: the CDK stack has no health check, metric filter or dashboard to conflict
        # with (nor a metric in Forma/Beta), and this template reaches the beta only through parameters. The service's log filter is
        # RUST_LOG when set, and a malformed one refuses to start (release E): nothing in infra/src sets it.
        for source in (ROOT / 'infra' / 'src').glob('*.ts'):
            text = source.read_text()
            for owned in ('aws-route53', 'AWS::Route53', 'CfnHealthCheck', 'Dashboard', 'MetricFilter', 'Forma/Beta', 'RUST_LOG'):
                self.assertFalse(owned in text, f'{source.name}: {owned}')


if __name__ == '__main__':
    unittest.main()
