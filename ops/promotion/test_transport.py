import unittest
from unittest.mock import Mock, patch
from ops.promotion.aws import AWS
from ops.promotion.registry import scan_complete
from ops.promotion.state import Refused


class APIError(Exception):
    def __init__(self, code): self.response={'Error':{'Code':code}}


class TransportTests(unittest.TestCase):
    def aws(self):
        aws=AWS.__new__(AWS);aws.clients={'ecs':Mock(),'ecr':Mock()};return aws
    def test_ecs_service_argument_is_not_python_api_selector(self):
        aws=self.aws();aws.call('ecs','update_service',service='exact-service',desiredCount=0)
        aws.clients['ecs'].update_service.assert_called_once_with(service='exact-service',desiredCount=0)
    def test_scan_not_found_then_in_progress_is_bounded_read_only_poll(self):
        aws=self.aws();client=aws.clients['ecr']
        client.describe_image_scan_findings.side_effect=[APIError('ScanNotFoundException'),{'imageScanStatus':{'status':'IN_PROGRESS'}},{'imageScanStatus':{'status':'COMPLETE'}}]
        with patch('ops.promotion.aws.time.sleep'):
            aws.wait('ecr','describe_image_scan_findings',scan_complete,timeout=30,retry_codes={'ScanNotFoundException'},repositoryName='fixed',imageId={'imageDigest':'fixed'})
        self.assertEqual(client.describe_image_scan_findings.call_count,3)
        self.assertEqual(len(client.method_calls),3)
    def test_unknown_or_terminal_scan_failure_is_not_retried(self):
        aws=self.aws();client=aws.clients['ecr'];client.describe_image_scan_findings.side_effect=APIError('AccessDeniedException')
        with self.assertRaises(APIError):aws.wait('ecr','describe_image_scan_findings',scan_complete,retry_codes={'ScanNotFoundException'})
        self.assertEqual(client.describe_image_scan_findings.call_count,1)
        with self.assertRaises(Refused):scan_complete({'imageScanStatus':{'status':'FAILED'}})
    def test_scan_never_started_times_out_without_starting_mutation(self):
        aws=self.aws();client=aws.clients['ecr'];client.describe_image_scan_findings.side_effect=APIError('ScanNotFoundException')
        with patch('ops.promotion.aws.time.monotonic',side_effect=[0,61]),self.assertRaises(Refused):
            aws.wait('ecr','describe_image_scan_findings',scan_complete,timeout=60,retry_codes={'ScanNotFoundException'})
        self.assertEqual(len(client.method_calls),1)

if __name__=='__main__':unittest.main()
