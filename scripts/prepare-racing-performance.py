"""Prepare public diagnostic input: GAME_FIXTURE CATALOG_RELEASE OUTPUT_DIR."""
import hashlib
import json
from pathlib import Path
import sys

if len(sys.argv) != 4:
    raise SystemExit(__doc__)
fixture, catalog, output = (Path(p).resolve() for p in sys.argv[1:])
reference = json.loads(fixture.read_text())
index = (catalog/'simulation/index.json').read_text()
resources = []
for resource in json.loads(index)['resources']:
    source = (catalog/resource['path']).resolve()
    source.relative_to(catalog)
    resources.append({'path': resource['path'], 'contents': source.read_text()})
bundle = {'manifest': (catalog/'catalog.json').read_text(),
          'release_identity': (catalog/'release.json').read_text(),
          'simulation_index': index,
          'presentation_index': (catalog/'presentation/index.json').read_text(),
          'resources': resources}
data = {'request_json': reference['input']['requestJson'],
        'profile_json': reference['input']['profileJson'],
        'catalog_bundle_json': json.dumps(bundle, separators=(',', ':'))}
output.mkdir(parents=True, exist_ok=True)
target = output/'input.json'
if target.exists():
    raise RuntimeError('Choose a fresh directory; existing inputs are not overwritten')
target.write_text(json.dumps(data, separators=(',', ':')))
print(json.dumps({'input_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
                  'request_sha256': hashlib.sha256(data['request_json'].encode()).hexdigest()}))
