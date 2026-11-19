"""Focused regression cases for the archive's repeated clock and incomplete marks."""
import tempfile
from pathlib import Path
import unittest
import xml.etree.ElementTree as ET
from import_beethoven9 import recording_clock, score_marks, opening_signatures, instrument_labels


class ArchiveImportTests(unittest.TestCase):
    def test_tuplet_and_tremolo_metadata_do_not_replace_the_published_duration(self):
        with tempfile.TemporaryDirectory() as folder:
            xml = Path(folder) / 'score.xml'
            xml.write_text('<score-partwise><part id="P51"><measure number="1"><note><pitch><step>A</step><octave>2</octave></pitch><duration>24</duration><type>quarter</type><dot/><time-modification><actual-notes>6</actual-notes><normal-notes>4</normal-notes><normal-type>16th</normal-type></time-modification><notations><ornaments><tremolo type="single">2</tremolo></ornaments></notations></note></measure></part></score-partwise>', encoding='utf8')
            notes = [{'part_id':'P51', 'measure':'1', 'voice':'1:1', 'beat':1, 'duration_beats':1, 'source_id':'s-p19-m1-n1'}]
            score_marks(xml, notes)
            self.assertEqual(notes[0]['duration_beats'], 1)
            self.assertEqual(notes[0]['source_id'], 's-p19-m1-n1')
            self.assertEqual(notes[0]['notation']['dots'], 1)
            self.assertEqual(notes[0]['notation']['rhythm'], {'time_modification':{'actual_notes':6, 'normal_notes':4, 'normal_type':'16th', 'normal_dots':0}, 'tremolos':[{'kind':'single', 'marks':2}]})
    def test_note_notation_keeps_original_identity_and_separates_hidden_from_muted(self):
        with tempfile.TemporaryDirectory() as folder:
            xml = Path(folder) / 'score.xml'
            xml.write_text('<score-partwise><part id="P2"><measure number="0"><note print-object="no" dynamics="0.00"><pitch><step>C</step><octave>5</octave></pitch><duration>2</duration><type>half</type></note><note print-object="no"><pitch><step>E</step><alter>-1</alter><octave>4</octave></pitch><duration>1</duration><type>eighth</type><dot/></note></measure></part></score-partwise>', encoding='utf8')
            notes = [{'part_id': 'P2', 'measure': '1', 'voice': '1:1', 'beat': i, 'source_id': f's-p2-m1-n{i + 1}'} for i in range(2)]
            self.assertEqual(score_marks(xml, notes), [])
            self.assertEqual(notes[0]['source_id'], 's-p2-m1-n1')
            self.assertFalse(notes[0]['notation']['visible'])
            self.assertEqual(notes[0]['notation']['playback_dynamics'], 0)
            self.assertEqual(notes[1]['notation']['written_pitch'], {'step': 'E', 'alter': -1, 'octave': 4})
            self.assertEqual(notes[1]['notation']['dots'], 1)
            self.assertIsNone(notes[1]['notation']['playback_dynamics'])
    def test_unperformed_supplement_inherits_pitch_and_is_an_inline_svg(self):
        from beethoven9_supplement import supplement_unperformed_score
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'music/score').mkdir(parents=True)
            xml = root / 'score.xml'
            xml.write_text('<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Clarinet</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef><transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose></attributes><note><rest/><duration>4</duration><type>whole</type></note></measure><measure number="X1"><note><pitch><step>D</step><alter>1</alter><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure><measure number="3"><note><rest/><duration>4</duration><type>whole</type></note></measure></part></score-partwise>', encoding='utf8')
            pages, objects = [], [{'measure': '1'}, {'measure': '3'}]
            report = supplement_unperformed_score(xml, root, pages, objects, {'P1': {'name': '单簧管', 'voices': ['1:1']}})
            self.assertEqual(report['measures'], [2])
            self.assertEqual(objects[-1]['midi_pitch'], 61)
            svg = (root / pages[0]['path']).read_text(encoding='utf8')
            self.assertTrue(svg.startswith('<svg '))
            self.assertIn('s-p1-m2-n1', svg)
            self.assertIn('本次录音未演奏', svg)

    def test_direction_offset_selects_the_corresponding_note(self):
        with tempfile.TemporaryDirectory() as folder:
            xml = Path(folder) / 'score.xml'
            xml.write_text('<score-partwise><part id="P2"><measure><attributes><divisions>24</divisions></attributes><direction><direction-type><words>ritard.</words></direction-type><offset>30</offset></direction></measure></part></score-partwise>', encoding='utf8')
            notes = [{"part_id": "P2", "measure": "1", "voice": "1:1", "beat": beat, "source_id": str(beat)} for beat in (0, 1.25)]
            mark = score_marks(xml, notes)[0]
            self.assertEqual(mark['beat'], 1.25)
            self.assertEqual(mark['score_object_ids'], ['1.25'])

    def test_opening_clef_inside_a_layer_supplies_the_page_signature(self):
        staff = ET.fromstring('<g><g class="layer"><g class="clef"><use transform="translate(10, 20)"/></g><g class="note"><use transform="translate(50, 20)"/></g></g></g>')
        self.assertIn('clef', opening_signatures(staff))

    def test_page_header_distinguishes_opening_clef_from_mid_measure_change(self):
        staff = ET.fromstring('<g><g class="clef"><use transform="translate(10, 20)"/></g><g class="layer"><g class="note"><use transform="translate(50, 20)"/></g></g><g class="keySig"><use transform="translate(100, 20)"/></g></g>')
        self.assertEqual(opening_signatures(staff), {'clef'})

    def test_transposing_instruments_remain_distinguishable(self):
        labels = {'P7:1': '单簧管1', 'P8:1': '单簧管1'}
        source = {'staves': {'P7:1': {'events': [{'pitch_label': 'B♭'}]}, 'P8:1': {'events': [{'pitch_label': 'A'}]}}}
        self.assertEqual(instrument_labels(labels, source), {'P7:1': '单簧管1（降B调）', 'P8:1': '单簧管1（A调）'})
    def test_opening_silence_uses_last_zero_anchor_and_clock_interpolates(self):
        clock = recording_clock({"sample_rate": 1000, "points": [[0, 0], [1200, 0], [2200, 1], [4200, 2]]})
        self.assertEqual(clock(0), 1200)
        self.assertEqual(clock(0.5), 1700)
        self.assertEqual(clock(2), 4200)

    def test_incomplete_metronome_keeps_original_number_as_text(self):
        with tempfile.TemporaryDirectory() as folder:
            xml = Path(folder) / "score.xml"
            xml.write_text('<score-partwise><part id="P2"><measure><direction><direction-type><words>Presto</words><metronome><beat-unit/><per-minute>116</per-minute></metronome></direction-type></direction></measure></part></score-partwise>', encoding='utf8')
            marks = score_marks(xml, [{"part_id": "P2", "measure": "1", "voice": "1:1", "beat": 0, "source_id": "s-p1-m1-n1"}])
            self.assertIsNone(marks[0]["value"]["metronome"])
            self.assertEqual(marks[0]["value"]["words"], ["Presto", "原谱节拍器：116（未注明拍值）"])


if __name__ == '__main__':
    unittest.main()
