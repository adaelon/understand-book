import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
import tempfile
import json
import io

spec = importlib.util.spec_from_file_location('watchdog', Path(__file__).with_name('operations-watchdog.py'))
w = importlib.util.module_from_spec(spec)
spec.loader.exec_module(w)


class WatchdogTests(unittest.TestCase):
    def ready(self):
        return {'model': {'status': 'ready'}, 'signals': {'failures': [], 'successful_scopes': {}}, 'last_probe': None}

    def tick(self, state, report=None, error=None, now=1):
        current = w.collect(report, error, state.get('incidents', {}), state.get('seen_failures', {}))
        w.transitions(state, current, now, 'owner@example.com', 'https://example.com')

    def test_outage_dedup_escalation_recovery_and_restart(self):
        state = {}
        self.tick(state, error='connection refused')
        self.tick(state, error='connection refused', now=2)
        self.assertEqual(len(state['pending']), 1)
        self.tick(state, self.ready(), now=3)
        self.assertEqual(len(state['pending']), 2)

    def test_resolved_failure_window_does_not_reopen_and_spam(self):
        r, state = self.ready(), {}
        r['signals']['failures'] = [{'stage': 'model', 'recovery_scope': 'model', 'error_code': 'TIMEOUT', 'users': 1, 'count': 1, 'last_at': 10}]
        self.tick(state, r)
        r['signals']['successful_scopes'] = {'model': 11}
        self.tick(state, r, now=12)
        self.tick(state, r, now=13)
        self.tick(state, r, now=14)
        self.assertEqual(len(state['pending']), 2)
        self.assertIn('恢复', state['pending'][1]['subject'])
        r = self.ready()
        r['model'] = {'status': 'warning', 'code': 'EXPIRES', 'message': 'soon'}
        self.tick(state, r)
        r['model']['status'] = 'critical'
        self.tick(state, r)
        self.assertIn('升级', state['pending'][-1]['subject'])
        self.tick(state, r)
        self.assertEqual(len(state['pending']), 4)

    def test_quiet_traffic_does_not_claim_recovery(self):
        r, state = self.ready(), {}
        r['signals']['failures'] = [{'stage': 'model', 'recovery_scope': 'model', 'error_code': 'TIMEOUT', 'users': 1, 'count': 1, 'last_at': 10}]
        self.tick(state, r)
        self.tick(state, self.ready(), now=1000)
        self.assertEqual(len(state['pending']), 1)
        r = self.ready(); r['signals']['successful_scopes'] = {'model': 1001}
        self.tick(state, r, now=1002)
        self.assertEqual(len(state['pending']), 2)

    def test_text_success_never_recovers_board_or_video_failure(self):
        for scope in ['board', 'video:book']:
            r, state = self.ready(), {}
            r['signals']['failures']=[{'stage':'delivery','error_code':'FAILED','recovery_scope':scope,'count':1,'users':1,'last_at':10}]
            self.tick(state,r)
            r['signals']['successful_scopes']={'model':11,'execution:book':11,'save:reader':11}
            self.tick(state,r)
            r['signals']['failures']=[]
            self.tick(state,r)
            self.assertEqual(len(state['pending']),1)
            r['signals']['successful_scopes'][scope]=12
            self.tick(state,r)
            self.assertEqual(len(state['pending']),2)
            self.assertEqual(state['incidents'],{})

    def test_web_expiry_and_long_pending_notify_and_recover(self):
        r,state=self.ready(),{}
        r['web']=[{'service_kind':'web_search','status':'warning','code':'EXPIRES','message':'expires'}]
        r['signals']['pending_charges']={'count':472,'reserved_micro_cny':128654186}
        self.tick(state,r)
        self.tick(state,r)
        self.assertEqual(len(state['pending']),2)
        r['web'][0]['status']='ready';r['signals']['pending_charges']['count']=0
        self.tick(state,r)
        self.assertEqual(len(state['pending']),4)
        self.assertEqual(state['incidents'],{})

    def test_failed_email_is_retained_and_retried_with_same_id(self):
        state = {}; self.tick(state, error='HTTP 503')
        identity = state['pending'][0]['id']
        snapshots = []
        def fail(event):
            raise RuntimeError('MAIL_HTTP_429')
        w.flush(state, lambda s: snapshots.append(len(s['pending'])), fail, 10)
        self.assertEqual(state['pending'][0]['id'], identity)
        self.assertEqual(state['mail_error'], 'MAIL_HTTP_429')
        sent = []
        w.flush(state, lambda s: None, lambda event: sent.append(event['id']) or 'receipt', 11)
        self.assertEqual(sent, [identity]); self.assertEqual(state['pending'], [])
        self.assertIsNone(state['mail_error']); self.assertEqual(state['last_email_at'], 11)

    def test_expiry_from_urgent_warning_to_outage_notifies_again(self):
        state, r = {}, self.ready()
        r['model'] = {'status': 'critical', 'code': 'EXPIRES_TODAY', 'message': 'expires today'}
        self.tick(state, r)
        self.assertIn('预警', state['pending'][-1]['subject'])
        r['model'] = {'status': 'unavailable', 'code': 'EXPIRED', 'message': 'expired'}
        self.tick(state, r, now=2)
        self.assertEqual(len(state['pending']), 2)
        self.assertIn('升级', state['pending'][-1]['subject'])
        self.tick(state, r, now=3)
        self.assertEqual(len(state['pending']), 2)
        r['model']['status'] = 'critical'
        self.tick(state, r, now=4)
        self.assertIn('恢复', state['pending'][-1]['subject'])

    def test_old_account_incident_rechecks_current_balance_before_recovery(self):
        previous = {'account:reader': {'severity': 'warning'}}
        for available in [0, 100]:
            class Opener:
                def open(inner, request, timeout):
                    path = w.urllib.parse.urlsplit(request.full_url).path
                    values = {'/api/auth/login': {}, '/api/auth/me': {'csrf_token': 'fixture'},
                        '/api/admin/operations': self.ready(), '/api/auth/logout': {},
                        '/api/admin/users/reader': {'current_allowance': {'validity_state': 'active', 'balance': {'available_micro_cny': available}}}}
                    return io.BytesIO(json.dumps(values[path]).encode())
            with tempfile.TemporaryDirectory() as directory:
                credentials = Path(directory)/'credentials.json'
                credentials.write_text('{"password":"fixture"}')
                with patch.object(w.urllib.request, 'build_opener', return_value=Opener()):
                    report = w.read_report('https://example.com', credentials, previous)
                self.assertEqual(bool(report['signals']['blocked_accounts']), available == 0)

    def test_disk_warns_before_state_and_user_results_cannot_be_saved(self):
        gib = 1024**3
        self.assertIsNone(w.disk_incident(100*gib, 20*gib))
        self.assertEqual(w.disk_incident(100*gib, 10*gib)['severity'], 'warning')
        self.assertEqual(w.disk_incident(100*gib, 3*gib)['severity'], 'critical')
        self.assertEqual(w.disk_incident(2*gib, 200*1024**2)['severity'], 'critical')


if __name__ == '__main__':
    unittest.main()
