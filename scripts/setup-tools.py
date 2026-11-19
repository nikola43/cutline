"""Install the official standalone yt-dlp into this project, verifying its release checksum."""
import hashlib, json, os, platform, urllib.request
from pathlib import Path

root = Path(__file__).resolve().parents[1] / '.tools' / 'bundled'
root.mkdir(parents=True, exist_ok=True)
asset = 'yt-dlp_macos' if platform.system() == 'Darwin' else ('yt-dlp.exe' if os.name == 'nt' else ('yt-dlp_linux_aarch64' if platform.machine().lower() in ('arm64','aarch64') else 'yt-dlp_linux'))
def fetch(url):
    request = urllib.request.Request(url, headers={'User-Agent': 'Cutline-build'})
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()
release = json.loads(fetch('https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest'))
assets = {a['name']: a['browser_download_url'] for a in release['assets']}
checksums = fetch(assets['SHA2-256SUMS']).decode()
expected = next(line.split()[0] for line in checksums.splitlines() if line.split()[-1].lstrip('*') == asset)
data = fetch(assets[asset])
assert hashlib.sha256(data).hexdigest() == expected, 'yt-dlp checksum mismatch'
target = root / ('yt-dlp.exe' if os.name == 'nt' else 'yt-dlp')
temp = target.with_suffix('.download')
temp.write_bytes(data)
temp.chmod(0o755)
temp.replace(target)
(root / 'release.json').write_text(json.dumps({'version': release['tag_name'], 'asset': asset, 'sha256': expected}, indent=2))
print('Installed verified yt-dlp', release['tag_name'], 'at', target)
