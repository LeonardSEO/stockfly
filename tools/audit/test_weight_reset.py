import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from weight_reset import input_hashes


class InputHashesTest(unittest.TestCase):
    def test_hashes_bind_graph_checkpoint_suite_maps_and_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            contents = {
                'neurons.bin': b'neurons', 'offsets.bin': b'offsets',
                'edge_src_blocks/0000.bin': b'sources',
                'edge_weight_blocks/0000.bin': b'weights',
                'model': b'model', 'suite': b'suite', 'binary': b'binary',
                'sensory-map.json': b'sensory', 'output-map.json': b'output',
            }
            for name, data in contents.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            digest = lambda name: hashlib.sha256(contents[name]).hexdigest()
            (root / 'manifest.json').write_text(json.dumps({
                'neurons_sha256': digest('neurons.bin'),
                'offsets_sha256': digest('offsets.bin'),
                'edge_blocks': [{'index': 0,
                                 'src_sha256': digest('edge_src_blocks/0000.bin'),
                                 'weight_sha256': digest('edge_weight_blocks/0000.bin')}],
            }))
            args = (root, root / 'model', root / 'suite', root, root / 'binary')
            first = input_hashes(*args)
            self.assertEqual(first, input_hashes(*args))
            for name, field in [('model', 'model_sha256'), ('suite', 'suite_sha256'),
                                ('binary', 'binary_sha256'), ('sensory-map.json', 'sensory_map_sha256'),
                                ('output-map.json', 'output_map_sha256')]:
                (root / name).write_bytes(b'changed')
                self.assertNotEqual(first[field], input_hashes(*args)[field])
                (root / name).write_bytes(contents[name])
            (root / 'edge_weight_blocks/0000.bin').write_bytes(b'corrupt')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                input_hashes(*args)


if __name__ == '__main__':
    unittest.main()
