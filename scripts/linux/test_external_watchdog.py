import importlib.util,unittest
from pathlib import Path
spec=importlib.util.spec_from_file_location('external',Path(__file__).with_name('external-watchdog.py'));m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class ExternalTests(unittest.TestCase):
 def test_outage_and_stale_backup_are_independent(self):
  now=200000;free=3*1024**3
  self.assertEqual(m.issues(now,None,now,free),{})
  self.assertEqual(set(m.issues(now,'connection refused',now,free)),{'site'})
  self.assertEqual(set(m.issues(now,None,now-36*3600-1,free)),{'backup'})
  self.assertEqual(set(m.issues(now,'timeout',None,1)),{'site','backup','disk'})
if __name__=='__main__':unittest.main()
