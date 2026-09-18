#!/usr/bin/env python3

import hashlib
from pathlib import Path
import tempfile
import unittest

from prepare_models import prepare_asset


class PrepareModelsTest(unittest.TestCase):
    def test_copy_mode_materializes_an_existing_same_hash_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source.sfckpt"
            destination = root / "cache" / "model.sfckpt"
            contents = b'{"format_version":1,"model_kind":"max-full","deltas":[]}'
            source.write_bytes(contents)
            expected_sha256 = hashlib.sha256(contents).hexdigest()

            prepare_asset(source, destination, "symlink", expected_sha256)
            self.assertTrue(destination.is_symlink())

            prepare_asset(source, destination, "copy", expected_sha256)

            self.assertTrue(destination.is_file())
            self.assertFalse(destination.is_symlink())
            self.assertEqual(destination.read_bytes(), contents)
            self.assertEqual(source.read_bytes(), contents)


if __name__ == "__main__":
    unittest.main()
