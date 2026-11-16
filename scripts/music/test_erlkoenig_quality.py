"""Regressions read against the user's actual Erlkoenig notation/recording."""
import json
from pathlib import Path
import unittest

WORK=Path(__file__).resolve().parents[2]/'.understand-book/erlkoenig'

class ErlkoenigQuality(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest=json.loads((WORK/'workspace/.build/music/import-candidate.json').read_text(encoding='utf-8'))
        cls.rhythm=json.loads((WORK/'transcription/rhythm-candidates.json').read_text(encoding='utf-8'))
        cls.alignment=json.loads((WORK/'transcription/audio-alignment.json').read_text(encoding='utf-8'))

    def test_dotted_half_and_ordinary_quarters_keep_their_written_duration(self):
        notes=self.rhythm['notes']
        half=[n for n in notes if n['measure']==6 and n['kind']=='head_half']
        self.assertEqual(len(half),2)
        self.assertTrue(all(n['duration_beats']==3 for n in half))
        byid={n['source_id']:n for n in notes}
        stop=next(a['start_ms'] for a in self.alignment['notes'] if a['measure']==6 and byid[a['source_id']]['beat']==3)
        aligned={a['source_id']:a for a in self.alignment['notes']}
        self.assertTrue(all(aligned[n['source_id']]['end_ms']==stop for n in half))
        ordinary=[n for n in notes if n['measure']==2 and n['staff']==2 and n['beat'] in [2,3]]
        self.assertEqual(len(ordinary),4)
        self.assertTrue(all(n['duration_beats']==1 for n in ordinary))

    def test_regular_twelve_attack_bars_are_not_skipped_or_stalled(self):
        for m in [28,32,108,112,114]:
            row=self.alignment['measures'][m-1]
            duration=row['end_ms']-row['start_ms']
            self.assertGreater(duration,600,f'measure {m} collapsed')
            self.assertLess(duration,4000,f'measure {m} stalled')

    def test_andante_belongs_to_the_last_beat_of_147(self):
        mark=next(m for m in self.manifest['score_marks'] if m['mark_id']=='direction-147')
        self.assertEqual(mark['beat'],3)
        anchors=[n for n in self.manifest['score_objects'] if n['source_id'] in mark['score_object_ids']]
        self.assertTrue(anchors and all(n['beat']==3 for n in anchors))

    def test_problem_region_matches_independently_read_video_and_attack_boundaries(self):
        for measure,seconds in [(112,177.06),(113,178.84),(114,180.28),(115,181.58)]:
            self.assertAlmostEqual(self.alignment['measures'][measure-1]['start_ms'],seconds*1000,delta=25)
            first=next(n for n in self.alignment['notes'] if n['measure']==measure)
            self.assertAlmostEqual(first['dtw_ms'],seconds*1000,delta=80)

    def test_source_rests_are_indexed_including_the_opening_and_final_bar(self):
        rests=[n for n in self.manifest['score_objects'] if n['rest']]
        self.assertEqual(len(rests),165)
        opening=[r for r in rests if r['measure']=='1']
        self.assertEqual([(r['beat'],r['duration_beats']) for r in opening],[(0,4)])
        final=[r for r in rests if r['measure']=='148']
        self.assertEqual(sorted(set((r['beat'],r['duration_beats']) for r in final)),[(1,1),(2,2)])
        self.assertEqual(len(final),4)
        self.assertTrue(all(r['midi_pitch'] is None for r in rests))

    def test_unobserved_attacks_are_not_advertised_as_accepted_note_following(self):
        for note in self.manifest['note_alignments']:
            if not note.get('unobserved'):continue
            accepted=[c for c in self.manifest['coverage'] if c['granularity']=='note' and c['status']=='accepted' and c['start_ms']<=note['start_ms']<c['end_ms']]
            self.assertFalse(accepted,f'unobserved {note["score_object_id"]} was accepted as observed')

if __name__=='__main__':unittest.main()
