"""Read-only guards for the isolated APP-140 invitation broker.

Called before either service is activated. No secret values are retrieved and
no mutation is performed here. Unknown or partial topology fails closed.
"""
import json


def require(value, message):
    if not value:
        raise ValueError('shared broker: ' + message)


def lifecycle(outputs, state, call, account='877573132739', region='us-east-1'):
    """A live read, before migration or release recording; no mutation."""
    require(state in ('stopped', 'healthy'), 'unknown lifecycle check')
    require(call('sts', 'get-caller-identity')['Account'] == account, 'AWS account differs')
    require('SharedBrokerServiceName' in outputs and 'CleanTrialOrganization' in outputs, 'clean topology absent')
    cluster = f"arn:aws:ecs:{region}:{account}:cluster/{outputs['ClusterName']}"
    names = ['SharedBrokerServiceName']
    if state == 'stopped':
        names.append('ServiceName')
    for name in names:
        arn = f"arn:aws:ecs:{region}:{account}:service/{outputs['ClusterName']}/{outputs[name]}"
        result = call('ecs', 'describe-services', '--cluster', cluster, '--services', arn)
        require(not result.get('failures') and len(result.get('services', [])) == 1, 'lifecycle service missing')
        service = result['services'][0]
        expected_definition = outputs['SharedBrokerTaskDefinition' if name == 'SharedBrokerServiceName' else 'TaskDefinition']
        require(service.get('serviceArn') == arn and service.get('clusterArn') == cluster
                and service.get('status') == 'ACTIVE' and service.get('taskDefinition') == expected_definition,
                'lifecycle service identity differs')
        expected = (0, 0, 0) if state == 'stopped' else (1, 1, 0)
        require(all(type(service.get(k)) is int for k in ('desiredCount', 'runningCount', 'pendingCount'))
                and tuple(service[k] for k in ('desiredCount', 'runningCount', 'pendingCount')) == expected,
                'services must be fully stopped before migration' if state == 'stopped' else 'broker is not healthy')
        if state == 'healthy':
            deployments = service.get('deployments', [])
            require(len(deployments) == 1 and deployments[0].get('status') == 'PRIMARY'
                    and deployments[0].get('taskDefinition') == expected_definition
                    and deployments[0].get('rolloutState') == 'COMPLETED', 'broker rollout is incomplete')
    return {'state': state, 'checked_services': len(names)}


def verify(config, outputs, workspace, cluster_arn, call):
    organization = config.get('clean_trial_organization_id', '')
    require(bool(organization) == ('SharedBrokerTaskDefinition' in outputs), 'deployment input and topology differ')
    if not organization:
        return None
    require(outputs.get('CleanTrialOrganization') == organization, 'organization differs from the reserved clean trial')
    environment = {e['name']: e['value'] for e in workspace.get('environment', [])}
    require(environment.get('GENUI_DEMO_MODE') == '0' and environment.get('GENUI_DEMO_ORGANIZATIONS') == '', 'demo mode must be off')
    require('GENUI_DISPLAY_NAMES_JSON' not in environment, 'sandbox persona labels are forbidden')
    service_arn = 'arn:aws:ecs:%s:%s:service/%s/%s' % (config['region'], config['account'], outputs['ClusterName'], outputs['SharedBrokerServiceName'])
    definition = call('ecs', 'describe-task-definition', '--task-definition', outputs['SharedBrokerTaskDefinition'])['taskDefinition']
    require(definition.get('taskDefinitionArn') == outputs['SharedBrokerTaskDefinition']
            and definition.get('runtimePlatform', {}).get('cpuArchitecture') == config['image_architecture']
            and definition.get('cpu') == '256' and definition.get('memory') == '512', 'task identity or bounded resources differ')
    require(definition.get('taskRoleArn') == outputs['SharedBrokerTaskRoleArn']
            and definition.get('executionRoleArn') == outputs['SharedBrokerExecutionRoleArn'], 'dedicated role identity differs')
    containers = definition.get('containerDefinitions', [])
    require(len(containers) == 1, 'exactly one isolated broker container is required')
    container = containers[0]
    require(container.get('name') == 'broker' and container.get('image') == config['operator_image_digest']
            and container.get('readonlyRootFilesystem') is True and container.get('user') == '65532:65532'
            and container.get('entryPoint') == ['python3', '-I', '-u', '/operator/app/operator/shared_invite_broker.py']
            and container.get('command', []) == [] and container.get('environmentFiles', []) == [], 'container boundary differs')
    expected_environment = {'AWS_REGION': config['region'], 'GENUI_SHARED_BROKER_BIND_ADDRESS': '0.0.0.0',
                            'GENUI_PROVIDER_MANAGEMENT_URL': config['provider_management_url']}
    require(container.get('environment') and len(container['environment']) == len(expected_environment)
            and {e['name']: e['value'] for e in container['environment']} == expected_environment, 'unexpected environment or credential channel')
    secret = config['provider_management_secret_arn']
    expected_secrets = {'GENUI_SHARED_BROKER_CONFIG_JSON': outputs['SharedBrokerConfigurationSecretArn'],
                        'GENUI_PROVIDER_MANAGEMENT_SECRET_KEY': secret + ':management_key::',
                        'GENUI_PROVIDER_MANAGEMENT_SECRET_TENANT_ID': secret + ':tenant_id::'}
    require(len(container.get('secrets', [])) == 3 and {s['name']: s['valueFrom'] for s in container['secrets']} == expected_secrets,
            'secret injection differs')
    schedule = call('scheduler', 'get-schedule', '--group-name', outputs['DeploymentExpiryGroup'], '--name', outputs['SharedBrokerExpiryScheduleName'])
    require(schedule.get('State') == 'ENABLED' and schedule.get('ScheduleExpressionTimezone') == 'UTC'
            and schedule.get('ScheduleExpression') == 'at(' + config['deployment_expires_at_utc'][:-1] + ')'
            and schedule.get('FlexibleTimeWindow') == {'Mode': 'OFF'}, 'expiry is not the reserved exact stop')
    target = schedule.get('Target', {})
    require(target.get('Arn') == 'arn:aws:scheduler:::aws-sdk:ecs:updateService'
            and target.get('RoleArn') == outputs['SharedBrokerExpiryRoleArn']
            and target.get('RetryPolicy') == {'MaximumEventAgeInSeconds': 900, 'MaximumRetryAttempts': 3}
            and json.loads(target.get('Input', '{}')) == {'Cluster': cluster_arn, 'Service': service_arn, 'DesiredCount': 0},
            'stop target differs')
    current = call('ecs', 'describe-services', '--cluster', cluster_arn, '--services', service_arn)
    require(not current.get('failures') and len(current.get('services', [])) == 1, 'service is unavailable')
    service = current['services'][0]
    require(service.get('serviceArn') == service_arn and service.get('clusterArn') == cluster_arn
            and service.get('status') == 'ACTIVE' and service.get('taskDefinition') == outputs['SharedBrokerTaskDefinition']
            and type(service.get('desiredCount')) is int and service['desiredCount'] in (0, 1), 'service identity or count differs')
    deployment = service.get('deploymentConfiguration', {})
    require(deployment.get('maximumPercent') == 100 and deployment.get('minimumHealthyPercent') == 0
            and service.get('enableExecuteCommand') is False, 'single-writer deployment boundary differs')
    return service_arn


if __name__ == '__main__':
    import argparse
    import importlib.util
    from pathlib import Path
    parser = argparse.ArgumentParser(description='Read-only clean-trial lifecycle check')
    parser.add_argument('--outputs', type=Path, required=True)
    parser.add_argument('--state', choices=('stopped', 'healthy'), required=True)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location('broker_preflight', Path(__file__).with_name('preflight.py'))
    preflight = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(preflight)
    document = json.loads(args.outputs.read_text())
    outputs = {item['OutputKey']: item['OutputValue'] for item in document['Stacks'][0]['Outputs']}
    print(json.dumps(lifecycle(outputs, args.state, preflight.aws_call, preflight.ACCOUNT, preflight.REGION)))
