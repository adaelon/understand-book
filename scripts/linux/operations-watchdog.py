#!/usr/bin/env python3
"""One independent business check; systemd schedules this even if the host server stops."""
import argparse
from datetime import datetime, timezone, timedelta
import http.cookiejar
import json
import os
import shutil
from pathlib import Path
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid


def atomic_json(path, value, mode=0o600):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix('.tmp')
    with temporary.open('w', encoding='utf-8') as out:
        json.dump(value, out, ensure_ascii=False, indent=2)
        out.flush()
        os.fsync(out.fileno())
    temporary.chmod(mode)
    temporary.replace(path)


def read_report(origin, credential_path, previous=None):
    credentials = json.loads(Path(credential_path).read_text())
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
    csrf = ''

    def call(path, data=None):
        req = urllib.request.Request(origin + path,
            data=None if data is None else json.dumps(data).encode(),
            headers={'Origin': origin, 'Content-Type': 'application/json',
                     'X-CSRF-Token': csrf, 'User-Agent': 'UnderstandBook-Operations/1'})
        try:
            with opener.open(req, timeout=12) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            raise RuntimeError(f'HTTP {error.code} at {path}') from None
    call('/api/auth/login', {'username': credentials.get('username', 'admin'), 'password': credentials['password']})
    try:
        csrf = call('/api/auth/me')['csrf_token']
        report = call('/api/admin/operations')
        if not isinstance(report.get('model'), dict) or 'signals' not in report:
            raise RuntimeError('OPERATIONS_RESPONSE_INVALID')
        # An older account incident must be checked explicitly after it leaves the recent window.
        blocked = report['signals'].setdefault('blocked_accounts', [])
        for key in previous or {}:
            if not key.startswith('account:'):
                continue
            owner = key.removeprefix('account:')
            if any(account['user_id'] == owner for account in blocked):
                continue
            account = call('/api/admin/users/' + urllib.parse.quote(owner, safe=''))
            allowance = account.get('current_allowance') or {}
            balance = allowance.get('balance') or {}
            if allowance.get('validity_state') != 'active' or not (balance.get('unlimited') or balance.get('available_micro_cny', 0) > 0):
                blocked.append({'user_id': owner})
        return report
    finally:
        if csrf:
            try:
                call('/api/auth/logout', {})
            except Exception:
                pass


def collect(report, error, previous, seen=None):
    """Absence of traffic is not evidence that a failed delivery recovered."""
    if error:
        retained = {k: v for k, v in previous.items() if k != 'site'}
        return {**retained, 'site': {'severity': 'critical', 'title': '网站或监测接口不可访问',
            'detail': error, 'action': '检查网站入口、服务进程及管理员监测凭据。'}}
    current = {}
    seen = seen or {}
    model = report['model']
    if model['status'] != 'ready':
        current['model'] = {'severity': 'critical' if model['status'] in ('unavailable', 'critical') else 'warning',
            'impact': 'blocked' if model['status'] == 'unavailable' else 'expiring',
            'title': model['message'], 'detail': model['code'],
            'action': '检查后台费率覆盖时间，核对当前供应商价格并续接配置；重新加载后验证一次真实交付。'}
    successes = report['signals'].get('successful_scopes', {})
    for service in report.get('web', []):
        if service['status'] != 'ready':
            current['web:' + service['service_kind']] = {
                'severity': 'critical' if service['status'] in ('unavailable', 'critical') else 'warning',
                'impact': 'blocked' if service['status'] == 'unavailable' else 'expiring',
                'title': service['message'], 'detail': service['service_kind'] + ' · ' + service['code'],
                'action': '核对供应商搜索和网页读取价格，续接有效费率并检查覆盖时间。'}
    pending = report['signals'].get('pending_charges', {})
    if pending.get('count', 0):
        current['pending_charges'] = {'severity': 'warning', 'title': '待核账已超过 24 小时，仍占用用户额度',
            'detail': f"{pending['count']} 条，预占 {pending['reserved_micro_cny']/1000000:.6f} 元",
            'action': '查看后台待核账和供应商回执，核账或经确认释放用户预占，保留成本未知的记录。'}
    for failure in report['signals'].get('failures', []):
        scope = failure.get('recovery_scope', 'manual')
        key = 'delivery:' + failure['stage'] + ':' + failure['error_code'] + ':' + scope
        success = successes.get(scope, 0)
        old = previous.get(key)
        if not old and failure['last_at'] <= seen.get(key, 0):
            continue
        if success > max(failure['last_at'], (old or {}).get('last_failure_at', 0)):
            continue
        current[key] = {'severity': 'critical' if failure['users'] >= 2 or failure['count'] >= 3 else 'warning',
            'title': '用户请求未成功交付',
            'detail': f"{failure['error_code']} · {failure['stage']} · 最近 15 分钟 {failure['count']} 次，影响 {failure['users']} 个账号",
            'last_failure_at': failure['last_at'], 'recovery_scope': scope, 'action': '在后台查看失败账号及回合，处理原因后验证成功交付。'}
        if old and old['severity'] == 'critical':
            current[key]['severity'] = 'critical'
    for key, incident in previous.items():
        if key.startswith('delivery:') and key not in current and successes.get(incident.get('recovery_scope', 'manual'), 0) <= incident.get('last_failure_at', 0):
            current[key] = incident
    probe = report.get('last_probe')
    for account in report['signals'].get('blocked_accounts', []):
        current['account:' + account['user_id']] = {'severity': 'warning', 'title': '用户请求被账号额度阻止',
            'detail': account['user_id'], 'action': '检查该账号额度余额及有效期。'}
    if report['signals'].get('stuck_requests', 0):
        current['stuck'] = {'severity': 'critical', 'title': '请求超过 30 分钟仍未结束',
            'detail': str(report['signals']['stuck_requests']) + ' 个请求待处理',
            'action': '检查后台排队、执行及保存状态，确认是否需要取消或恢复。'}
    if probe and probe['outcome'] != 'delivered':
        current['probe'] = {'severity': 'critical', 'title': '最近一次真实交付验证失败',
            'detail': probe.get('error_code') or probe['outcome'],
            'action': '检查验证回合并重新进行真实交付测试。'}
    return current


def transitions(state, current, now, recipient, origin, prefix=''):
    previous = state.setdefault('incidents', {})
    pending = state.setdefault('pending', [])
    for key in sorted(previous.keys() | current.keys()):
        old, new = previous.get(key), current.get(key)
        if old is None:
            kind, incident = '故障' if new['severity'] == 'critical' and new.get('impact') != 'expiring' else '预警', new
        elif new is None:
            kind, incident = '恢复', old
        elif (old['severity'] != 'critical' and new['severity'] == 'critical') or (old.get('impact') == 'expiring' and new.get('impact') == 'blocked'):
            kind, incident = '升级', new
        elif old.get('impact') == 'blocked' and new.get('impact') == 'expiring':
            kind, incident = '恢复', {**new, 'title': '服务调用已恢复，费率仍即将到期'}
        else:
            continue
        at = datetime.fromtimestamp(now, timezone(timedelta(hours=8))).strftime('%Y-%m-%d %H:%M:%S 香港时间')
        pending.append({'id': 'ub-ops-' + str(uuid.uuid4()), 'created_at': now,
            'to': [recipient], 'subject': f'{prefix}[Understand Book · {kind}] {incident["title"]}',
            'text': f'{kind}：{incident["title"]}\n时间：{at}\n详情：{incident["detail"]}\n'
                + ('恢复依据：当前配置或服务检查通过；交付故障需有同一能力或相关范围内更新的成功交付记录。\n' if kind == '恢复' else f'建议处理：{incident["action"]}\n')
                + f'查看后台：{origin}/admin/monitor\n\n此邮件由独立监测服务自动发送。'} )
    state['incidents'] = current
    seen = state.setdefault('seen_failures', {})
    for key, incident in current.items():
        if key.startswith('delivery:'):
            seen[key] = max(seen.get(key, 0), incident.get('last_failure_at', 0))
    state['last_check_at'] = now


def disk_incident(total, free):
    remaining = free / total
    if remaining > 0.10 and free > 1024**3:
        return None
    return {'severity': 'critical' if remaining <= 0.03 or free <= 256*1024**2 else 'warning',
        'title': '服务数据磁盘空间不足',
        'detail': f'可用 {free/1024**3:.2f} GiB，占总容量 {remaining:.1%}',
        'action': '检查磁盘占用，清理确认可删除的构建产物，保留用户数据与必要备份。'}


def send_mail(event, env):
    payload = {key: event[key] for key in ('to', 'subject', 'text')}
    payload['from'] = env['UNDERSTAND_BOOK_MAIL_FROM']
    request = urllib.request.Request(env.get('UNDERSTAND_BOOK_MAIL_ENDPOINT') or 'https://api.resend.com/emails',
        data=json.dumps(payload).encode(), headers={'Authorization': 'Bearer ' + env['UNDERSTAND_BOOK_MAIL_API_KEY'],
            'Content-Type': 'application/json', 'User-Agent': 'UnderstandBook-Operations/1', 'Idempotency-Key': event['id']})
    try:
        with urllib.request.urlopen(request, timeout=15) as response:
            result = json.load(response)
            if not result.get('id'):
                raise RuntimeError('MAIL_RESPONSE_INVALID')
            return result['id']
    except urllib.error.HTTPError as error:
        raise RuntimeError(f'MAIL_HTTP_{error.code}') from None


def flush(state, save, sender, now):
    # Persist each transition before any network operation, and each receipt before the next one.
    save(state)
    for event in list(state['pending'])[:6]:
        try:
            receipt = sender(event)
        except Exception as error:
            state['mail_error'] = str(error)
            save(state)
            return
        state['pending'].remove(event)
        state['last_email_at'] = now
        state['last_email_id'] = receipt
        state['mail_error'] = None
        save(state)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--origin', default='https://understandbook.top')
    parser.add_argument('--credentials', default='/opt/understand-book/adm10-admin-credentials.json')
    parser.add_argument('--state', default='/opt/understand-book/operations/watchdog-state.json')
    parser.add_argument('--status', required=True)
    args = parser.parse_args()
    recipient = os.environ['UNDERSTAND_BOOK_ALERT_EMAIL']
    state_path = Path(args.state)
    state = json.loads(state_path.read_text()) if state_path.exists() else {}
    now = int(time.time())
    try:
        report, error = read_report(args.origin, args.credentials, state.get('incidents', {})), None
    except Exception as failure:
        report, error = None, str(failure)
    current = collect(report, error, state.get('incidents', {}), state.get('seen_failures', {}))
    disk = shutil.disk_usage(Path(args.status).parent)
    current.pop('disk', None)
    issue = disk_incident(disk.total, disk.free)
    if issue:
        current['disk'] = issue
    transitions(state, current, now, recipient, args.origin)
    flush(state, lambda value: atomic_json(state_path, value), lambda event: send_mail(event, os.environ), now)
    status = {key: state.get(key) for key in ('last_check_at', 'last_email_at', 'last_email_id', 'mail_error', 'incidents')}
    status.update(recipient=recipient, pending_emails=len(state['pending']), interval_seconds=60,
                  scope='same_host', check_error=error)
    # Only non-secret status is readable by the app; the mail queue remains root-only.
    atomic_json(args.status, status, 0o644)
    print(json.dumps({'checked_at': now, 'incident_count': len(current), 'pending_emails': len(state['pending']),
                      'mail_error': state.get('mail_error'), 'check_error': error}, ensure_ascii=False))


if __name__ == '__main__':
    main()
