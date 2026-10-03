#!/usr/bin/env python3
"""Compile the layered macOS icon and regenerate native fallbacks (Xcode 26+)."""
from pathlib import Path
import struct
import subprocess
import tempfile

branding = Path(__file__).resolve().parent
sizes = [(16, "icp4"), (32, "icp5"), (64, "icp6"), (128, "ic07"), (256, "ic08"), (512, "ic09"), (1024, "ic10")]
images = []
with tempfile.TemporaryDirectory() as temporary:
    source = Path(temporary) / "app-icon.png"
    compiled = Path(temporary) / "compiled"
    compiled.mkdir()
    subprocess.run(
        ["xcrun", "actool", str(branding / "topo.icon"),
         "--compile", str(compiled), "--platform", "macosx",
         "--minimum-deployment-target", "13.0", "--app-icon", "topo",
         "--output-partial-info-plist", str(Path(temporary) / "icon-info.plist"),
         "--output-format", "human-readable-text", "--warnings", "--errors"],
        check=True,
    )
    (branding / "topo.icns").write_bytes((compiled / "topo.icns").read_bytes())
    subprocess.run(
        ["sips", "-s", "format", "png", str(compiled / "topo.icns"), "--out", str(source)],
        check=True, stdout=subprocess.DEVNULL,
    )
    for size, _ in sizes:
        output = Path(temporary) / f"{size}.png"
        subprocess.run(
            ["sips", "-z", str(size), str(size), str(source), "--out", str(output)],
            check=True, stdout=subprocess.DEVNULL,
        )
        images.append(output.read_bytes())

offset = 6 + 16 * 5
entries = []
for (size, _), data in zip(sizes[:5], images[:5]):
    entries.append(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset))
    offset += len(data)
(branding / "topo.ico").write_bytes(struct.pack("<HHH", 0, 1, 5) + b"".join(entries) + b"".join(images[:5]))
(branding / "topo-app-icon.png").write_bytes(images[5])

svg = (branding / "topo-logo.svg").read_text()
tile = '''  <defs>
    <linearGradient id="tile-fill" x1="256" y1="50" x2="256" y2="462" gradientUnits="userSpaceOnUse">
      <stop stop-color="#1B1C20"/>
      <stop offset="1" stop-color="#111216"/>
    </linearGradient>
  </defs>
  <path fill="url(#tile-fill)" stroke="white" stroke-opacity="0.14" d="M151 50 H361 C434 50 462 78 462 151 V361 C462 434 434 462 361 462 H151 C78 462 50 434 50 361 V151 C50 78 78 50 151 50 Z"/>
'''
svg = svg.replace('  <g fill=', tile + '  <g transform="translate(51.2 51.2) scale(0.8)" fill=', 1)
(branding.parents[1] / "promo/assets/favicon.svg").write_text(svg)
