"""Read-only activation guards, with a fake AWS transport recording every call."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('broker_activation', Path(__file__).parents[1]/'scripts/shared_broker_activation.py')
broker = importlib.util.module_from_spec(spec); spec.loader.exec_module(broker)


class BrokerActivation(unittest.TestCase):
    def fixture(self):
        config = {'clean_trial_organization_id':'00000000-0000-4000-8000-0000000000e1', 'account':'877573132739', 'region':'us-east-1',
                  'image_architecture':'ARM64', 'operator_image_digest':'registry/operator@sha256:'+'a'*64,
                  'provider_management_secret_arn':'arn:aws:secretsmanager:us-east-1:877573132739:secret:management-AbCdEf',
                  'provider_management_url':'https://auth.example.test','deployment_expires_at_utc':'2026-11-30T23:00:00Z'}
        outputs = {'ClusterName':'cluster','CleanTrialOrganization':config['clean_trial_organization_id'], 'SharedBrokerServiceName':'broker',
                   'SharedBrokerTaskDefinition':'broker-task:1', 'SharedBrokerTaskRoleArn':'broker-role', 'SharedBrokerExecutionRoleArn':'broker-execution',
                   'SharedBrokerConfigurationSecretArn':'broker-config', 'DeploymentExpiryGroup':'group', 'SharedBrokerExpiryScheduleName':'broker-expiry', 'SharedBrokerExpiryRoleArn':'broker-stop-role'}
        cluster='arn:aws:ecs:us-east-1:877573132739:cluster/cluster'
        service='arn:aws:ecs:us-east-1:877573132739:service/cluster/broker'
        workspace={'environment':[{'name':'GENUI_DEMO_MODE','value':'0'},{'name':'GENUI_DEMO_ORGANIZATIONS','value':''}]}
        container={'name':'broker','image':config['operator_image_digest'],'user':'65532:65532','readonlyRootFilesystem':True,
                   'entryPoint':['python3','-I','-u','/operator/app/operator/shared_invite_broker.py'],'command':[],
                   'environment':[{'name':k,'value':v} for k,v in {'AWS_REGION':'us-east-1','GENUI_SHARED_BROKER_BIND_ADDRESS':'0.0.0.0','GENUI_PROVIDER_MANAGEMENT_URL':config['provider_management_url']}.items()],
                   'secrets':[{'name':k,'valueFrom':v} for k,v in {'GENUI_SHARED_BROKER_CONFIG_JSON':'broker-config','GENUI_PROVIDER_MANAGEMENT_SECRET_KEY':config['provider_management_secret_arn']+':management_key::','GENUI_PROVIDER_MANAGEMENT_SECRET_TENANT_ID':config['provider_management_secret_arn']+':tenant_id::'}.items()]}
        definition={'taskDefinitionArn':'broker-task:1','runtimePlatform':{'cpuArchitecture':'ARM64'},'cpu':'256','memory':'512',
                    'taskRoleArn':'broker-role','executionRoleArn':'broker-execution','containerDefinitions':[container]}
        schedule={'State':'ENABLED','ScheduleExpressionTimezone':'UTC','ScheduleExpression':'at(2026-11-30T23:00:00)', 'FlexibleTimeWindow':{'Mode':'OFF'},
                  'Target':{'Arn':'arn:aws:scheduler:::aws-sdk:ecs:updateService','RoleArn':'broker-stop-role','RetryPolicy':{'MaximumEventAgeInSeconds':900,'MaximumRetryAttempts':3},
                            'Input':json.dumps({'Cluster':cluster,'Service':service,'DesiredCount':0})}}
        current={'serviceArn':service,'clusterArn':cluster,'status':'ACTIVE','taskDefinition':'broker-task:1','desiredCount':0,'enableExecuteCommand':False,
                 'deploymentConfiguration':{'maximumPercent':100,'minimumHealthyPercent':0}}
        calls=[]
        def call(*args):
            calls.append(args)
            if args[:2]==('ecs','describe-task-definition'): return {'taskDefinition':definition}
            if args[:2]==('scheduler','get-schedule'): return schedule
            if args[:2]==('ecs','describe-services'): return {'services':[current]}
            raise AssertionError('unexpected mutation or secret read')
        return config,outputs,workspace,cluster,call,calls,definition,schedule,current
    def test_exact_qualified_topology_is_read_only(self):
        c,o,w,cluster,call,calls,*_=self.fixture()
        self.assertEqual(broker.verify(c,o,w,cluster,call),'arn:aws:ecs:us-east-1:877573132739:service/cluster/broker')
        self.assertEqual(len(calls),3)
        self.assertIsNone(broker.verify({}, {}, {}, cluster,call))
        with self.assertRaises(ValueError): broker.verify({},o,w,cluster,call)
    def test_drift_and_credential_or_single_writer_bypass_refuse(self):
        mutations=[lambda c,o,w,d,s,r:w['environment'][0].update(value='1'),
                   lambda c,o,w,d,s,r:o.update(CleanTrialOrganization='another'),
                   lambda c,o,w,d,s,r:d.update(cpu='2048'),
                   lambda c,o,w,d,s,r:d.update(taskRoleArn='ordinary-app-role'),
                   lambda c,o,w,d,s,r:d['containerDefinitions'][0]['environment'].append({'name':'PGPASSWORD','value':'forbidden'}),
                   lambda c,o,w,d,s,r:d['containerDefinitions'][0]['secrets'][0].update(valueFrom='other-config'),
                   lambda c,o,w,d,s,r:d['containerDefinitions'][0].update(readonlyRootFilesystem=False),
                   lambda c,o,w,d,s,r:s.update(State='DISABLED'),
                   lambda c,o,w,d,s,r:s['Target'].update(Input=json.dumps({'Cluster':'other','Service':'other','DesiredCount':0})),
                   lambda c,o,w,d,s,r:r.update(desiredCount=2),
                   lambda c,o,w,d,s,r:r.update(enableExecuteCommand=True),
                   lambda c,o,w,d,s,r:r['deploymentConfiguration'].update(maximumPercent=200)]
        for mutate in mutations:
            c,o,w,cluster,call,calls,d,s,r=self.fixture(); mutate(c,o,w,d,s,r)
            with self.subTest(mutation=mutations.index(mutate)), self.assertRaises(ValueError): broker.verify(c,o,w,cluster,call)
            self.assertTrue(all('update-service' not in call for call in calls))

    def test_migration_requires_both_services_drained_and_release_requires_completed_broker(self):
        _, outputs, _, cluster, _, _, _, _, current = self.fixture()
        outputs.update(ServiceName='app', TaskDefinition='app-task:1')
        current.update(runningCount=0, pendingCount=0)
        app = dict(current, serviceArn=cluster.replace(':cluster/', ':service/')+'/app', taskDefinition='app-task:1')
        calls = []
        def call(*args):
            calls.append(args)
            if args[:2] == ('sts', 'get-caller-identity'): return {'Account': '877573132739'}
            if args[:2] == ('ecs', 'describe-services'):
                return {'services': [app if args[-1].endswith('/app') else current]}
            raise AssertionError('lifecycle check must be read-only')
        self.assertEqual(broker.lifecycle(outputs, 'stopped', call), {'state': 'stopped', 'checked_services': 2})
        for service in (current, app):
            for field in ('desiredCount', 'runningCount', 'pendingCount'):
                service[field] = 1
                with self.assertRaises(ValueError): broker.lifecycle(outputs, 'stopped', call)
                service[field] = 0
        with self.assertRaises(ValueError): broker.lifecycle(outputs, 'healthy', call)
        current.update(desiredCount=1, runningCount=1,
                       deployments=[{'status': 'PRIMARY', 'taskDefinition': 'broker-task:1', 'rolloutState': 'COMPLETED'}])
        self.assertEqual(broker.lifecycle(outputs, 'healthy', call), {'state': 'healthy', 'checked_services': 1})
        for change in ({'rolloutState': 'IN_PROGRESS'}, {'taskDefinition': 'old-task:1'}, {'status': 'ACTIVE'}):
            before = dict(current['deployments'][0]); current['deployments'][0].update(change)
            with self.assertRaises(ValueError): broker.lifecycle(outputs, 'healthy', call)
            current['deployments'][0] = before
        self.assertTrue(all(args[1] in ('get-caller-identity', 'describe-services') for args in calls))
