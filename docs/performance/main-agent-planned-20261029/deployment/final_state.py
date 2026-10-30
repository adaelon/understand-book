from pathlib import Path
import json,os,runpy,subprocess
base=Path('/opt/understand-book');stage=base/'acceptance/main-agent-planned-20261029';release=base/'releases/main-agent-planned-20261029'
run=lambda *args:subprocess.check_output(list(map(str,args)),text=True).strip()
pid=run('systemctl','show','understand-book-multi','-p','MainPID','--value')
assert os.readlink('/proc/'+pid+'/exe')==str(release/'target/release/server')
assert run('systemctl','show','understand-book-multi','-p','NRestarts','--value')=='0'
worker=json.loads((base/'data/multi-reader-to-tb-20261012/presentation-sandbox.json').read_text())['worker'];assert worker==str(release/'target/release/presentation_worker')
state=runpy.run_path(str(base/'acceptance/capacity-fixes-20261027/ops.py'))['state']()
report={'status':'passed','release':str(release),'pid':pid,'restart_count':0,'worker_matches_release':True,'state':state,'temporary_swap_removed':not (stage/'build.swap').exists()}
(stage/'final-state.json').write_text(json.dumps(report,indent=2));print(json.dumps(report))
