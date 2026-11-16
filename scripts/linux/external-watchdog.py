#!/usr/bin/env python3
"""External availability and off-host backup freshness; runs on the retired host."""
import json, os, runpy, shutil, time, urllib.request, urllib.error
from pathlib import Path
BASE=Path('/opt/understand-book/operations-external')
def check_site(origin):
    headers={'Origin':origin,'User-Agent':'UnderstandBook-External-Operations/1'}
    request=urllib.request.Request(origin+'/api/auth/me',headers=headers)
    try:
        with urllib.request.urlopen(request,timeout=15) as response:
            if response.status!=200:raise RuntimeError('Unexpected authentication response')
    except urllib.error.HTTPError as error:
        if error.code!=401:raise RuntimeError('HTTP '+str(error.code)) from None
    with urllib.request.urlopen(urllib.request.Request(origin+'/',headers=headers),timeout=15) as response:
        if response.status!=200 or b'<html' not in response.read(8192).lower():raise RuntimeError('Website entry is unavailable')
def issues(now,error,backup_at,free):
    current={}
    if error:current['site']={'severity':'critical','title':'外部监测无法访问网站','detail':error,'action':'检查主机网络、HTTPS 入口及主服务。此检查来自另一台服务器。'}
    if backup_at is None or now-backup_at>36*3600:
        current['backup']={'severity':'critical','title':'异机备份已超过 36 小时未更新','detail':'尚无成功备份' if backup_at is None else '最后完成时间：'+time.strftime('%Y-%m-%d %H:%M:%S UTC',time.gmtime(backup_at)),'action':'检查主机备份服务日志、SSH 连接和备份磁盘。'}
    if free<1024**3:current['disk']={'severity':'critical','title':'异机备份磁盘剩余不足 1 GiB','detail':f'{free/1024**3:.2f} GiB','action':'检查备份保留策略和旧机磁盘容量，保留最新可恢复快照。'}
    return current
def main():
    w=runpy.run_path(str(BASE/'operations-watchdog.py'))
    now=int(time.time());origin='https://understandbook.top';path=BASE/'state.json';state=json.loads(path.read_text()) if path.exists() else {}
    try:check_site(origin);error=None
    except Exception as e:error=str(e)
    snapshots=Path('/opt/understand-book/backups/production-restic/snapshots')
    last=max((int(p.stat().st_mtime) for p in snapshots.iterdir() if p.is_file()),default=None) if snapshots.exists() else None
    marker=BASE/'maintenance-until'
    maintenance=int(marker.read_text()) if marker.exists() else 0
    current=issues(now,None if now<maintenance else error,last,shutil.disk_usage(BASE).free)
    if now<maintenance and 'site' in state.get('incidents',{}):current['site']=state['incidents']['site']
    w['transitions'](state,current,now,os.environ['UNDERSTAND_BOOK_ALERT_EMAIL'],origin,prefix='[异机] ')
    w['flush'](state,lambda s:w['atomic_json'](path,s),lambda event:w['send_mail'](event,os.environ),now)
    w['atomic_json'](BASE/'status.json',{'checked_at':now,'backup_at':last,'incidents':current,'pending_emails':len(state['pending']),'mail_error':state.get('mail_error')})
if __name__=='__main__':main()
