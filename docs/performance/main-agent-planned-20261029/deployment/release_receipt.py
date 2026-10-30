from pathlib import Path
import json
base=Path('/opt/understand-book');s=base/'acceptance/main-agent-planned-20261029';r=base/'releases/main-agent-planned-20261029'
built=json.loads((s/'build-completion.json').read_text());assert built['status']=='built'
source=json.loads((s/'release-source.json').read_text())
receipt={'release':r.name,'previous_release':source['previous_release'],'local_source_commit':source['source_commit'],'includes_uncommitted_MA0_MA5':True,'online_source':'reader-outline-20261029 snapshot plus scoped merged source','source_manifest':str(s/'release-source.json'),'web_mode':'multi-user','build':built,'binaries':{n:(r/'target/release'/n).stat().st_size for n in ['server','manage_reader','publish_book','reader_maintenance','presentation_worker','video_audio_review']}}
p=r/'multi-reader-build.txt';p.unlink(missing_ok=True);p.write_text(json.dumps(receipt,indent=2));print(json.dumps({'release':r.name,'binary_count':len(receipt['binaries']),'web_mode':'multi-user'}))
