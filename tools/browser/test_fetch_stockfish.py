import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("fetch_stockfish", Path(__file__).with_name("fetch_stockfish.py"))
fetch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetch)


class AtomicStockfishAssetsTest(unittest.TestCase):
    def test_offline_copy_and_verified_cache_preserve_pinned_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source, out = root / "source", root / "out"
            source.mkdir()
            data = b"verified fixture asset"
            (source / "engine.wasm").write_bytes(data)
            files = {"engine.wasm": ("https://example.invalid/engine.wasm", fetch.digest(data))}
            with patch.dict(fetch.FILES, files, clear=True), patch.object(fetch, "urlopen", side_effect=AssertionError("offline must not fetch")):
                fetch.materialize(out, source, True)
                self.assertEqual((out / "engine.wasm").read_bytes(), data)
                fetch.materialize(out, None, True)
                self.assertEqual((out / "engine.wasm").read_bytes(), data)
                self.assertEqual(sorted(p.name for p in out.iterdir()), ["NOTICE.txt", "engine.wasm", "sources.json"])

    def test_corrupt_offline_source_does_not_replace_existing_destination(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source, out = root / "source", root / "out"
            source.mkdir()
            out.mkdir()
            (source / "engine.wasm").write_bytes(b"corrupt")
            (out / "engine.wasm").write_bytes(b"previous bytes")
            with patch.dict(fetch.FILES, {"engine.wasm": ("unused", fetch.digest(b"expected"))}, clear=True):
                with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                    fetch.materialize(out, source, True)
            self.assertEqual((out / "engine.wasm").read_bytes(), b"previous bytes")
            self.assertEqual([p.name for p in out.iterdir()], ["engine.wasm"])

    def test_write_interruption_and_replace_failure_leave_old_file_and_clean_temp(self):
        for operation in ["fsync", "replace"]:
            with self.subTest(operation=operation), tempfile.TemporaryDirectory() as temp:
                destination = Path(temp) / "engine.wasm"
                destination.write_bytes(b"previous bytes")
                with patch.object(fetch.os, operation, side_effect=OSError("interrupted")):
                    with self.assertRaisesRegex(OSError, "interrupted"):
                        fetch.atomic_write(destination, b"new verified bytes")
                self.assertEqual(destination.read_bytes(), b"previous bytes")
                self.assertEqual([p.name for p in Path(temp).iterdir()], ["engine.wasm"])


if __name__ == "__main__":
    unittest.main()
