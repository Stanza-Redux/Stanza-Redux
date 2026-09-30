"""Bundle unmodified Readium stylesheets, embedding their two optional fonts for offline CSP."""
from pathlib import Path
import base64, json, re
root = Path(__file__).resolve().parents[1] / 'resource/assets/reader'
css = {}
for kind in ('before','default','after'):
    text = (root / f'readium-css/ReadiumCSS-{kind}.css').read_text()
    def embed(match):
        name = match[1]
        data = (root / 'readium-css' / name).read_bytes()
        mime = 'font/otf' if name.endswith('.otf') else 'font/ttf'
        return 'url("data:' + mime + ';base64,' + base64.b64encode(data).decode() + '")'
    css[kind] = re.sub(r'url\("(fonts/[^\"]+)"\)',embed,text)
(root / 'styles.js').write_text('// Readium CSS: BSD-3-Clause, see READIUM-LICENSE.txt and font licenses.\nwindow.stanzaCSS=' + json.dumps(css) + ';\n')
