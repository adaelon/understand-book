from pathlib import Path
import subprocess,tarfile,json
S=Path('/opt/understand-book/acceptance/cloud-20261021')
archive=S/'LibreOffice_26.8.1_Linux_x86-64_rpm.tar.gz'
url='https://mirrors.nju.edu.cn/tdf/libreoffice/stable/26.8.1/rpm/x86_64/LibreOffice_26.8.1_Linux_x86-64_rpm.tar.gz'
try:
 subprocess.run(['curl','--fail','--location','--retry','2','--output',str(archive),url],check=True)
 dest=S/'libreoffice-rpm';dest.mkdir(exist_ok=True)
 with tarfile.open(archive) as f:f.extractall(dest,filter='data')
 packages=sorted(dest.glob('*/RPMS/*.rpm'));assert packages
 subprocess.run(['dnf','-y','install',*[str(p) for p in packages]],check=True)
 version=subprocess.check_output(['/opt/libreoffice26.8/program/soffice','--version'],text=True,cwd=S).strip()
 (S/'office-installed.json').write_text(json.dumps({'version':version,'source':url,'executable':'/opt/libreoffice26.8/program/soffice'},indent=2))
 (S/'office.exit').write_text('0\n')
except BaseException:
 (S/'office.exit').write_text('1\n');raise
