"""Offline lifecycle campaign: real bundle, CAS journal, budget and boundary code.

Only AWS transport, current GitHub reads and public TLS transport are synthetic.
No cloud API is called. Failures occur after accepted mutations to model lost ACKs.
"""
import copy
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from infra.test.preflight_test import always_on
from ops.promotion import guards
from ops.promotion.runner import Runner, preflight
from ops.promotion.github import validate_summary
from ops.promotion.state import Store, Refused, encoded
from ops.promotion.test_artifacts import bundle, archive
from ops.promotion.artifacts import inspect_oci
from ops.promotion.test_state import MemoryS3
from ops.promotion.test_github import approval

NOW = datetime(2026, 11, 2, 6, tzinfo=timezone.utc)


def fixture(manifest):
    config, ledger = always_on(NOW)
    config['image_digest'] = f'{guards.ACCOUNT}.dkr.ecr.{guards.REGION}.amazonaws.com/{guards.REPOS["service"]}@sha256:' + 'a'*64
    ledger, reserved = preflight().reserve(config, ledger, NOW, activating=True)
    outputs = dict(ClusterName='beta', ServiceName='main', SharedBrokerServiceName='broker', RetainedIdentity='untouched')
    definitions = {}
    for key, role in guards.ROLE_KEYS.items():
        arn = f'arn:aws:ecs:{guards.REGION}:{guards.ACCOUNT}:task-definition/{key}:1'
        outputs[key] = arn
        definitions[key] = dict(taskDefinitionArn=arn, family=key, revision=1, cpu='256', memory='512',
            runtimePlatform={'cpuArchitecture':'ARM64','operatingSystemFamily':'LINUX'},
            taskRoleArn=f'arn:aws:iam::{guards.ACCOUNT}:role/fixed-task', executionRoleArn=f'arn:aws:iam::{guards.ACCOUNT}:role/fixed-execution',
            containerDefinitions=[dict(name='workspace',image=reserved['cloudformation_parameters'][guards.IMAGE_KEYS[role]],
                environment=[{'name':'GENUI_SIGNIN_ALIASES_JSON','value':'{"aliases":{}}'}], secrets=[{'name':'AUTH','valueFrom':'fixed-secret-ref'}])])
    schedules = {n:dict(Name=n,GroupName='expiry',State='ENABLED',ScheduleExpressionTimezone='UTC',
        ScheduleExpression='at('+config['deployment_expires_at_utc'][:-1]+')',Target={'Arn':'fixed-stop-target','Input':json.dumps({'service':n,'count':0})}) for n in ['main','broker']}
    template={'Resources':{k:{'Type':'AWS::ECS::TaskDefinition'} for k in guards.ROLE_KEYS}}
    source=manifest['source']
    return dict(schema_version=1,revision=0,active=None,completed={},ledger=ledger,
        authority=dict(stack_id=f'arn:aws:cloudformation:{guards.REGION}:{guards.ACCOUNT}:stack/{guards.STACK}/fixture',
            prepare_role_arn=f'arn:aws:iam::{guards.ACCOUNT}:role/prepare',execute_role_arn=f'arn:aws:iam::{guards.ACCOUNT}:role/execute',
            execution_role_arn=f'arn:aws:iam::{guards.ACCOUNT}:role/restricted-cfn'),
        policy=dict(shorten_expiry_hours=2,schema_version=19),
        baseline=dict(template=template,definitions=definitions,schedules=schedules,
            schema_manifest_sha256=source['schema_manifest_sha256'],deployment_contract_sha256=source['deployment_contract_sha256']),
        live=dict(candidate_sha='f'*40,window_id=config['window_id'],config=config,parameters=reserved['cloudformation_parameters'],
            outputs=outputs,stack_execution_role_arn=f'arn:aws:iam::{guards.ACCOUNT}:role/old-cfn',
            registry={r:dict(index_digest='sha256:'+c*64,manifest_digest='sha256:'+c*64,findings=[]) for r,c in zip(guards.REPOS,'abc')}))


class FakeAWS:
    def __init__(self, state, s3, manifest):
        self.original=copy.deepcopy(state);self.state=copy.deepcopy(state);self.s3=s3;self.manifest=manifest
        self.phase='prepare';self.calls=[];self.fail=None;self.counts={'main':1,'broker':1};self.cs=None
        self.images={};self.tags={};self.layers={};self.uploads={};self.running_health='HEALTHY'
    def identity(self):return {'Account':guards.ACCOUNT,'Arn':f'arn:aws:sts::{guards.ACCOUNT}:assumed-role/{self.phase}/fixture'}
    def stack(self):
        live=self.state['live']
        return dict(StackId=self.state['authority']['stack_id'],StackName=guards.STACK,StackStatus='UPDATE_COMPLETE',RoleARN=live['stack_execution_role_arn'],
            Parameters=[{'ParameterKey':k,'ParameterValue':v} for k,v in live['parameters'].items()],Outputs=[{'OutputKey':k,'OutputValue':v} for k,v in live['outputs'].items()])
    def services(self):
        out=self.state['live']['outputs'];cluster=f'arn:aws:ecs:{guards.REGION}:{guards.ACCOUNT}:cluster/beta'
        return [dict(serviceName=n,serviceArn=f'arn:aws:ecs:{guards.REGION}:{guards.ACCOUNT}:service/beta/{n}',clusterArn=cluster,
            taskDefinition=out['TaskDefinition' if n=='main' else 'SharedBrokerTaskDefinition'],status='ACTIVE',desiredCount=c,runningCount=c,pendingCount=0,
            deployments=[{'rolloutState':'COMPLETED'}],loadBalancers=[{'targetGroupArn':'fixed-'+n}]) for n,c in self.counts.items()]
    def wait(self, service, operation, predicate, **args):
        args.pop('timeout',None);args.pop('retry_codes',None)
        value=self.call(service,operation,**args)
        assert predicate(value), 'synthetic state did not settle'
        return value
    def call(self, service, operation, /, **args):
        self.calls.append((service,operation,copy.deepcopy(args)))
        value=self._call(service,operation,args)
        key=(operation,args.get('service'),args.get('desiredCount'))
        if self.fail == key or self.fail == operation:
            self.fail=None;raise TimeoutError('synthetic accepted mutation lost response')
        return copy.deepcopy(value)
    def _call(self,s,o,a):
        if s=='s3':return self.s3.put_object(**a)
        if s=='cloudformation':
            if o=='describe_stacks':return {'Stacks':[self.stack()]}
            if o=='get_template':return {'TemplateBody':self.state['baseline']['template']}
            if o=='list_change_sets':return {'Summaries':[]}
            if o=='create_change_set':
                arn=f'arn:aws:cloudformation:{guards.REGION}:{guards.ACCOUNT}:changeSet/{a["ChangeSetName"]}/fixture'
                self.cs=dict(StackName=guards.STACK,Status='CREATE_COMPLETE',ExecutionStatus='AVAILABLE',ChangeSetId=arn,Parameters=a['Parameters'],
                    Changes=[{'ResourceChange':{'Action':'Modify','LogicalResourceId':k,'ResourceType':'AWS::ECS::TaskDefinition','Replacement':'True'}} for k in guards.ROLE_KEYS])
                self.create=copy.deepcopy(a);return {'Id':arn}
            if o=='describe_change_set':return self.cs
            if o=='execute_change_set':
                assert self.counts=={'main':0,'broker':0}
                live=self.state['live'];live['parameters']=guards.parameters(self.cs);live['stack_execution_role_arn']=self.create['RoleARN']
                for key,role in guards.ROLE_KEYS.items():
                    td=self.state['baseline']['definitions'][key];td['revision']=2;td['taskDefinitionArn']=td['taskDefinitionArn'][:-1]+'2'
                    td['containerDefinitions'][0]['image']=live['parameters'][guards.IMAGE_KEYS[role]];live['outputs'][key]=td['taskDefinitionArn']
                for schedule in self.state['baseline']['schedules'].values():schedule['ScheduleExpression']='at('+live['parameters']['DeploymentExpiresAtUtc'][:-1]+')'
                self.cs['ExecutionStatus']='EXECUTE_COMPLETE';return {}
        if s=='ecs':
            if o=='describe_task_definition':return {'taskDefinition':next(d for d in self.state['baseline']['definitions'].values() if d['taskDefinitionArn']==a['taskDefinition'])}
            if o=='describe_services':return {'services':self.services(),'failures':[]}
            if o=='list_tasks':return {'taskArns':['task-'+a['serviceName']]}
            if o=='describe_tasks':
                name=a['tasks'][0].removeprefix('task-');role='service' if name=='main' else 'operator'
                key='TaskDefinition' if name=='main' else 'SharedBrokerTaskDefinition';live=self.state['live']
                registry=self.manifest['images'] if live['outputs'][key].endswith(':2') else live['registry']
                return {'tasks':[{'lastStatus':'RUNNING','taskDefinitionArn':live['outputs'][key],
                    'containers':[{'image':live['parameters'][guards.IMAGE_KEYS[role]],'imageDigest':registry[role]['index_digest'],
                        'lastStatus':'RUNNING','healthStatus':self.running_health}]}],'failures':[]}
            if o=='update_service':
                self.counts[a['service']]=a['desiredCount']
                if a['desiredCount']==1:
                    persisted=json.loads(self.s3.objects['beta/state.json']);assert persisted['active']['phase']=='activation-intended'
                    assert 'activation_requested_at_utc' in persisted['active']
                return {'service':next(x for x in self.services() if x['serviceName']==a['service'])}
        if s=='scheduler':return self.state['baseline']['schedules'][a['Name']]
        if s=='elbv2':return {'TargetHealthDescriptions':[{'TargetHealth':{'State':'healthy'}}]}
        if s=='ecr':
            repo=a['repositoryName'] if 'repositoryName' in a else a['repositoryNames'][0]
            if o=='describe_repositories':return {'repositories':[{'imageTagMutability':'IMMUTABLE','imageScanningConfiguration':{'scanOnPush':True}}]}
            if o=='batch_get_image':
                ident=a['imageIds'][0];d=ident.get('imageDigest') or self.tags.get((repo,ident.get('imageTag')))
                if (repo,d) not in self.images:return {'images':[],'failures':[{'failureCode':'ImageNotFound'}]}
                return {'images':[{'imageId':{'imageDigest':d},'imageManifest':self.images[repo,d]}]}
            if o=='batch_check_layer_availability':return {'layers':[{'layerDigest':d,'layerAvailability':'AVAILABLE'} for d in a['layerDigests'] if (repo,d) in self.layers]}
            if o=='initiate_layer_upload':
                upload=str(len(self.uploads));self.uploads[upload]=b'';return {'uploadId':upload}
            if o=='upload_layer_part':
                raw=self.uploads[a['uploadId']];assert len(raw)==a['partFirstByte'];self.uploads[a['uploadId']]+=a['layerPartBlob'];return {}
            if o=='complete_layer_upload':
                raw=self.uploads[a['uploadId']];assert 'sha256:'+hashlib.sha256(raw).hexdigest()==a['layerDigests'][0]
                self.layers[repo,a['layerDigests'][0]]=raw;return {}
            if o=='put_image':
                raw=a['imageManifest'];assert 'sha256:'+hashlib.sha256(raw.encode()).hexdigest()==a['imageDigest']
                self.images[repo,a['imageDigest']]=raw
                if 'imageTag' in a:self.tags[repo,a['imageTag']]=a['imageDigest']
                return {'image':{'imageId':{'imageDigest':a['imageDigest']}}}
            if o=='describe_image_scan_findings':return {'imageScanStatus':{'status':'COMPLETE'},'imageScanFindings':{'findings':[]}}
        raise AssertionError((s,o,a))


class LifecycleTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.directory=Path(self.tmp.name);self.manifest=bundle(self.directory)
        self.initial=fixture(self.manifest);self.s3=MemoryS3(self.initial);self.store=Store(self.s3,'bucket','beta')
        self.aws=FakeAWS(self.initial,self.s3,self.manifest);self.runner=Runner(self.aws,self.store,clock=lambda:NOW,probes=lambda:{'synthetic':True})
        self.request=dict(candidate_sha=self.manifest['candidate_sha'],qualified_artifact_manifest_sha256=hashlib.sha256((self.directory/'manifest.json').read_bytes()).hexdigest(),
            authorization=dict(actor='fixture',run_id='42',run_attempt='1',repository=guards.REPOSITORY,workflow_ref=guards.WORKFLOW))
        self.current=patch('ops.promotion.candidate.check_current');self.current_mock=self.current.start()
        self.addCleanup(self.current.stop);self.addCleanup(self.tmp.cleanup)
    def prepared(self):
        result=self.runner.prepare(self.request,self.directory);validate_summary(result,self.request['candidate_sha'])
        self.execute=dict(self.request,promotion_id=result['promotion_id'],packet_sha256=result['packet_sha256']);self.aws.phase='execute'
        return result
    def approval(self):
        return approval()
    def run_execute(self):
        return self.runner.execute(self.execute,self.directory,approval())
    def test_predecessor_may_be_unsettled_until_activation(self):
        from ops.promotion.state import Refused
        self.store.load()
        def unsettled():
            rows = FakeAWS.services(self.aws)
            for row in rows:
                row['runningCount'] = 0
                row['pendingCount'] = 1
                row['deployments'] = [{'rolloutState': 'IN_PROGRESS'}]
            return rows
        self.aws.services = unsettled
        self.runner.boundary(1, require_settled=False)
        with self.assertRaises(Refused):
            self.runner.boundary(1)

    def test_real_prepare_packet_execute_summary_and_root_manifest_publish(self):
        self.prepared();result=self.run_execute();validate_summary(result,self.request['candidate_sha'])
        final=self.store.load();self.assertIsNone(final['active']);self.assertEqual(final['live']['candidate_sha'],self.request['candidate_sha'])
        self.assertEqual(final['baseline']['definitions']['TaskDefinition']['revision'],2)
        self.assertEqual(set(self.initial['ledger']['windows']) | {result['promotion_id']},set(final['ledger']['windows']))
        self.assertEqual(self.aws.counts,{'main':1,'broker':1});self.assertEqual(self.current_mock.call_count,3)
        mutations=[(o,a.get('service'),a.get('desiredCount')) for s,o,a in self.aws.calls if o in ['update_service','execute_change_set']]
        self.assertEqual(mutations,[('update_service','main',0),('update_service','broker',0),('execute_change_set',None,None),('update_service','broker',1),('update_service','main',1)])
        self.assertEqual(len(self.aws.tags),3)
        self.assertTrue(all(i['index_digest']==i['manifest_digest'] for i in self.manifest['images'].values()))
    def test_index_root_publication_and_unique_arm_child_readback(self):
        for role in guards.REPOS:
            path=self.directory/(role+'.oci.tar');qualified=archive(path,indexed=True)
            self.manifest['images'][role]=dict(inspect_oci(path),qualified_id=qualified,id_kind='config')
        (self.directory/'manifest.json').write_bytes(encoded(self.manifest))
        self.request['qualified_artifact_manifest_sha256']=hashlib.sha256((self.directory/'manifest.json').read_bytes()).hexdigest()
        self.prepared();result=self.run_execute();validate_summary(result,self.request['candidate_sha'])
        self.assertTrue(all(i['index_digest']!=i['manifest_digest'] for i in self.manifest['images'].values()))
        self.assertEqual(len(self.aws.images),6)
    def test_wrong_running_digest_after_approval_cannot_stop(self):
        self.prepared();original=self.aws._call
        def transport(s,o,a):
            result=original(s,o,a)
            if o=='describe_tasks': result['tasks'][0]['containers'][0]['imageDigest']='sha256:'+'0'*64
            return result
        self.aws._call=transport
        with self.assertRaises(Refused):self.run_execute()
        self.assertFalse(any(o=='update_service' for _,o,_ in self.aws.calls))
    def test_expired_or_changed_disposition_after_approval_cannot_stop(self):
        self.prepared()
        with patch('ops.promotion.runner.guard_scan_delta', side_effect=Refused('expired disposition')):
            with self.assertRaises(Refused): self.run_execute()
        self.assertFalse(any(o in ['update_service', 'execute_change_set'] for _,o,_ in self.aws.calls))
        self.assertEqual(self.store.load()['active']['phase'], 'prepared')
    def test_real_disposition_expiring_during_final_network_checks_cannot_stop(self):
        from ops.promotion.registry import scan_disposition
        disposition, _ = scan_disposition()
        contract = disposition['deployment_contract_sha256']
        self.manifest['source']['deployment_contract_sha256'] = contract
        candidate_path = self.directory/'candidate.json'
        candidate = json.loads(candidate_path.read_bytes())
        candidate['source'] = self.manifest['source']
        candidate_path.write_bytes(encoded(candidate))
        self.manifest['reports']['candidate.json'] = hashlib.sha256(candidate_path.read_bytes()).hexdigest()
        (self.directory/'manifest.json').write_bytes(encoded(self.manifest))
        self.request['qualified_artifact_manifest_sha256'] = hashlib.sha256((self.directory/'manifest.json').read_bytes()).hexdigest()
        self.initial['baseline']['deployment_contract_sha256'] = contract
        for role, entry in disposition['roles'].items():
            self.initial['live']['registry'][role]['manifest_digest'] = entry['predecessor_manifest_digest']
        self.s3.objects['beta/state.json'] = encoded(self.initial)
        self.aws.state = copy.deepcopy(self.initial); self.aws.original = copy.deepcopy(self.initial)
        original = self.aws._call
        by_repository = {guards.REPOS[role]: entry['findings'] for role, entry in disposition['roles'].items()}
        def transport(service, operation, args):
            if operation == 'describe_image_scan_findings' and args['repositoryName'] in by_repository:
                return {'imageScanStatus': {'status': 'COMPLETE'}, 'imageScanFindings': {'findings': by_repository[args['repositoryName']]}}
            return original(service, operation, args)
        self.aws._call = transport
        result = self.prepared()
        for role in guards.REPOS:
            self.assertEqual(result['review']['scan'][role]['reviewed_count'], len(disposition['roles'][role]['findings']))
        def final_network_read(_):
            self.runner.clock = lambda: datetime.fromisoformat(disposition['expires_at_utc'].replace('Z', '+00:00'))
        self.current_mock.side_effect = final_network_read
        with self.assertRaises(Refused): self.run_execute()
        self.assertFalse(any(o in ['update_service', 'execute_change_set'] for _,o,_ in self.aws.calls))
        self.assertEqual(self.store.load()['active']['phase'], 'prepared')
    def test_wrong_packet_and_approval_cannot_stop_services(self):
        self.prepared();self.execute['packet_sha256']='0'*64
        with self.assertRaises(Refused):self.run_execute()
        self.execute['packet_sha256']=self.store.load()['active']['packet_sha256']
        with self.assertRaises(ValueError):self.runner.execute(self.execute,self.directory,{})
        self.assertFalse(any(o=='update_service' for _,o,_ in self.aws.calls));self.assertEqual(self.aws.counts,{'main':1,'broker':1})
    def test_unhealthy_predecessor_can_prepare_and_strict_boundary_still_refuses(self):
        self.aws.running_health=None
        result=self.runner.prepare(self.request,self.directory)
        self.assertEqual(result['status'],'prepared')
        self.assertFalse(any(o=='update_service' for _,o,_ in self.aws.calls))
        with self.assertRaises(Refused):self.runner.boundary(1)
    def test_lost_ack_after_stop_cf_or_activation_holds_lease_and_never_replays(self):
        for fault in [('update_service','main',0),'execute_change_set',('update_service','broker',1)]:
            with self.subTest(fault=fault):
                # New exact fixture per isolated deployment.
                self.setUp();self.prepared();self.aws.fail=fault
                with self.assertRaises(TimeoutError):self.run_execute()
                persisted=self.store.load();self.assertIsNotNone(persisted['active'])
                self.assertEqual(persisted['active']['journal'][-1]['status'],'intent')
                calls=len(self.aws.calls)
                with self.assertRaises(Refused):self.run_execute()
                self.assertEqual(len(self.aws.calls),calls)

if __name__=='__main__':unittest.main()
