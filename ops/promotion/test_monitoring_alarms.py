"""Pins the standalone trial alarm template (ops/monitoring/alarms.json): parameters, resources, thresholds, wiring."""
import json
from pathlib import Path
import re
import unittest

TEMPLATE = Path(__file__).resolve().parents[1] / 'monitoring' / 'alarms.json'
TOPIC = {'Ref': 'AlarmTopic'}


def strings(value):
    if isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)
    elif isinstance(value, str):
        yield value


class AlarmTemplateTests(unittest.TestCase):
    def setUp(self):
        self.template = json.loads(TEMPLATE.read_text())
        self.resources = self.template['Resources']

    def test_parameters_are_exactly_the_identifiers_and_the_address(self):
        self.assertEqual(self.template['AWSTemplateFormatVersion'], '2010-09-09')
        self.assertEqual(set(self.template['Parameters']), {'NotificationEmail', 'LoadBalancerFullName', 'TargetGroupFullName', 'ClusterName',
                                                            'ServiceName', 'DBInstanceIdentifier', 'MonthlyBudgetUsd'})
        for name, parameter in self.template['Parameters'].items():
            with self.subTest(parameter=name):
                self.assertEqual(parameter['Type'], 'Number' if name == 'MonthlyBudgetUsd' else 'String')
                self.assertNotIn('Default', parameter) if name != 'MonthlyBudgetUsd' else self.assertEqual(parameter['Default'], 248)
                if parameter['Type'] == 'String':
                    self.assertIn('AllowedPattern', parameter)
        self.assertTrue(re.fullmatch(self.template['Parameters']['LoadBalancerFullName']['AllowedPattern'], 'app/Genui-LoadB-ABCDEF123456/0123456789abcdef'))
        self.assertTrue(re.fullmatch(self.template['Parameters']['TargetGroupFullName']['AllowedPattern'], 'targetgroup/Genui-Targe-ABCDEF123456/0123456789abcdef'))
        self.assertFalse(re.fullmatch(self.template['Parameters']['LoadBalancerFullName']['AllowedPattern'], 'arn:aws:elasticloadbalancing:us-east-1:1:loadbalancer/app/x/0123456789abcdef'))
        self.assertTrue(re.fullmatch(self.template['Parameters']['NotificationEmail']['AllowedPattern'], 'victor@example.test'))
        self.assertFalse(re.fullmatch(self.template['Parameters']['NotificationEmail']['AllowedPattern'], 'not an address'))

    def test_resources_are_the_topic_its_subscription_five_alarms_and_one_budget(self):
        types = {name: r['Type'] for name, r in self.resources.items()}
        self.assertEqual(types, {'AlarmTopic': 'AWS::SNS::Topic', 'AlarmTopicPolicy': 'AWS::SNS::TopicPolicy', 'EmailSubscription': 'AWS::SNS::Subscription',
                                 'LoadBalancer5xxAlarm': 'AWS::CloudWatch::Alarm', 'Target5xxAlarm': 'AWS::CloudWatch::Alarm',
                                 'UnhealthyTargetsAlarm': 'AWS::CloudWatch::Alarm', 'NoRunningTaskAlarm': 'AWS::CloudWatch::Alarm',
                                 'DatabaseFreeStorageAlarm': 'AWS::CloudWatch::Alarm', 'MonthlyBudget': 'AWS::Budgets::Budget'})
        # Standalone: nothing here is a resource promotion protects or the stack owns.
        for forbidden in ('AWS::RDS::', 'AWS::ElasticLoadBalancingV2::', 'AWS::ECS::', 'AWS::KMS::', 'AWS::S3::', 'AWS::SecretsManager::', 'AWS::IAM::'):
            self.assertFalse(any(t.startswith(forbidden) for t in types.values()), forbidden)
        subscription = self.resources['EmailSubscription']['Properties']
        self.assertEqual((subscription['Protocol'], subscription['Endpoint'], subscription['TopicArn']), ('email', {'Ref': 'NotificationEmail'}, TOPIC))
        statement = self.resources['AlarmTopicPolicy']['Properties']['PolicyDocument']['Statement']
        self.assertEqual([(s['Principal'], s['Action'], s['Resource']) for s in statement], [({'Service': 'cloudwatch.amazonaws.com'}, 'sns:Publish', TOPIC)])
        self.assertEqual(statement[0]['Condition'], {'StringEquals': {'aws:SourceAccount': {'Ref': 'AWS::AccountId'}}})
        self.assertEqual(self.template['Outputs']['AlarmTopicArn']['Value'], TOPIC)

    def test_every_alarm_notifies_the_topic_on_alarm_and_recovery_with_its_pinned_threshold(self):
        expected = {
            'LoadBalancer5xxAlarm': ('AWS/ApplicationELB', 'HTTPCode_ELB_5XX_Count', 'Sum', 300, 1, 5, 'GreaterThanOrEqualToThreshold', 'notBreaching',
                                     [('LoadBalancer', 'LoadBalancerFullName')]),
            'Target5xxAlarm': ('AWS/ApplicationELB', 'HTTPCode_Target_5XX_Count', 'Sum', 300, 1, 5, 'GreaterThanOrEqualToThreshold', 'notBreaching',
                               [('LoadBalancer', 'LoadBalancerFullName')]),
            'UnhealthyTargetsAlarm': ('AWS/ApplicationELB', 'UnHealthyHostCount', 'Maximum', 60, 2, 1, 'GreaterThanOrEqualToThreshold', 'breaching',
                                      [('LoadBalancer', 'LoadBalancerFullName'), ('TargetGroup', 'TargetGroupFullName')]),
            'NoRunningTaskAlarm': ('AWS/ECS', 'CPUUtilization', 'SampleCount', 60, 3, 1, 'LessThanThreshold', 'breaching',
                                   [('ClusterName', 'ClusterName'), ('ServiceName', 'ServiceName')]),
            'DatabaseFreeStorageAlarm': ('AWS/RDS', 'FreeStorageSpace', 'Minimum', 300, 1, 2 * 1024 ** 3, 'LessThanThreshold', 'breaching',
                                         [('DBInstanceIdentifier', 'DBInstanceIdentifier')]),
        }
        alarms = {name: r['Properties'] for name, r in self.resources.items() if r['Type'] == 'AWS::CloudWatch::Alarm'}
        self.assertEqual(set(alarms), set(expected))
        for name, (namespace, metric, statistic, period, evaluations, threshold, operator, missing, dimensions) in expected.items():
            with self.subTest(alarm=name):
                alarm = alarms[name]
                self.assertEqual((alarm['Namespace'], alarm['MetricName'], alarm['Statistic'], alarm['Period'], alarm['EvaluationPeriods'],
                                  alarm['Threshold'], alarm['ComparisonOperator'], alarm['TreatMissingData']),
                                 (namespace, metric, statistic, period, evaluations, threshold, operator, missing))
                self.assertEqual([(d['Name'], d['Value']) for d in alarm['Dimensions']], [(n, {'Ref': p}) for n, p in dimensions])
                self.assertEqual((alarm['AlarmActions'], alarm['OKActions']), ([TOPIC], [TOPIC]))
                self.assertEqual(alarm['AlarmName'], {'Fn::Sub': '${AWS::StackName}-' + re.sub(r'(?<!^)(?=[A-Z0-9])', '-', name.removesuffix('Alarm')).lower().replace('-5-xx', '-5xx')})
                self.assertGreater(len(alarm['AlarmDescription']), 60)

    def test_budget_notifies_at_80_and_100_actual_and_100_forecast_by_email(self):
        budget = self.resources['MonthlyBudget']['Properties']
        self.assertEqual(budget['Budget']['BudgetType'], 'COST')
        self.assertEqual(budget['Budget']['TimeUnit'], 'MONTHLY')
        self.assertEqual(budget['Budget']['BudgetLimit'], {'Amount': {'Ref': 'MonthlyBudgetUsd'}, 'Unit': 'USD'})
        notices = [(n['Notification']['NotificationType'], n['Notification']['Threshold'], n['Notification']['ThresholdType'], n['Notification']['ComparisonOperator'])
                   for n in budget['NotificationsWithSubscribers']]
        self.assertEqual(notices, [('ACTUAL', 80, 'PERCENTAGE', 'GREATER_THAN'), ('ACTUAL', 100, 'PERCENTAGE', 'GREATER_THAN'),
                                   ('FORECASTED', 100, 'PERCENTAGE', 'GREATER_THAN')])
        for notice in budget['NotificationsWithSubscribers']:
            self.assertEqual(notice['Subscribers'], [{'SubscriptionType': 'EMAIL', 'Address': {'Ref': 'NotificationEmail'}}])

    def test_no_private_identifier_is_embedded(self):
        text = TEMPLATE.read_text()
        for private in ('877573132739', 'arn:aws:', 'amazonaws.com/', 'dkr.ecr', '/Users/'):
            self.assertNotIn(private, text.replace('cloudwatch.amazonaws.com', ''))
        self.assertIsNone(re.search(r'(?<![0-9])[0-9]{12}(?![0-9])', text.replace('2147483648', '')))
        for value in strings(self.template):
            self.assertNotRegex(value, r'^subnet-|^sg-|^vpc-|^i-')


if __name__ == '__main__':
    unittest.main()
