"""Release gating must reject the production renderer outage before MCP writes."""
import runpy
from pathlib import Path
import unittest

require_ready = runpy.run_path(str(Path(__file__).with_name('smoke-mcp-release.py')))['require_presentation_ready']


class PresentationReadinessTests(unittest.TestCase):
    def test_ready(self):
        require_ready(dict(reason='ready', authoring=True, plot=True, animation=True, preview=True))

    def test_probe_failed_or_missing_capability(self):
        for capability in ({}, dict(reason='probe_failed', authoring=False, plot=False, animation=False, preview=False)):
            with self.subTest(capability=capability), self.assertRaises(AssertionError):
                require_ready(capability)

    def test_partial_availability(self):
        with self.assertRaises(AssertionError):
            require_ready(dict(reason='ready', authoring=True, plot=True, animation=True, preview=False))


if __name__ == '__main__':
    unittest.main()
