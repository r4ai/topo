#!/usr/bin/env python3
"""Regenerate native icons and favicon from the adopted logo (macOS sips)."""
from pathlib import Path
import struct
import subprocess
import tempfile

branding = Path(__file__).resolve().parent
sizes = [(16, "icp4"), (32, "icp5"), (64, "icp6"), (128, "ic07"), (256, "ic08"), (512, "ic09")]
images = []
with tempfile.TemporaryDirectory() as temporary:
    for size, _ in sizes:
        output = Path(temporary) / f"{size}.png"
        subprocess.run(
            ["sips", "-z", str(size), str(size), str(branding / "topo-logo-preview.png"), "--out", str(output)],
            check=True, stdout=subprocess.DEVNULL,
        )
        images.append(output.read_bytes())

chunks = [tag.encode() + struct.pack(">I", len(data) + 8) + data for (_, tag), data in zip(sizes, images)]
body = b"".join(chunks)
(branding / "topo.icns").write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)

offset = 6 + 16 * 5
entries = []
for (size, _), data in zip(sizes[:5], images[:5]):
    entries.append(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset))
    offset += len(data)
(branding / "topo.ico").write_bytes(struct.pack("<HHH", 0, 1, 5) + b"".join(entries) + b"".join(images[:5]))

svg = (branding / "topo-logo.svg").read_text()
svg = svg.replace('  <g fill=', '  <rect width="512" height="512" rx="96" fill="#111216"/>\n  <g fill=', 1)
(branding.parents[1] / "promo/assets/favicon.svg").write_text(svg)
