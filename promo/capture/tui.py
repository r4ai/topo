# Runs the real `topo tui` in a pseudo terminal and dumps its screen as JSON.
# Usage: uv run --with pyte tui.py <topo-bin> <workspace> <out.json> <cols> <rows> <step>...
# A step is `keys:<text>`, `sh:<command>`, `wait:<ms>` or `dump:<name>`.
import fcntl
import json
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time

import pyte

topo, ws, out, cols, rows, *steps = sys.argv[1:]
cols, rows = int(cols), int(rows)

pid, fd = pty.fork()
if pid == 0:
    os.chdir(ws)
    os.environ["TERM"] = "xterm-256color"
    os.execv(topo, [topo, "tui"])
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))

screen = pyte.Screen(cols, rows)
stream = pyte.ByteStream(screen)


def pump(seconds):
    end = time.time() + seconds
    while time.time() < end:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if ready:
            try:
                stream.feed(os.read(fd, 65536))
            except OSError:
                return


def dump():
    lines = []
    for y in range(rows):
        row = screen.buffer[y]
        runs = []
        for x in range(cols):
            c = row[x]
            style = [c.fg, c.bg, c.bold, c.reverse]
            if runs and runs[-1]["s"] == style:
                runs[-1]["t"] += c.data
            else:
                runs.append({"t": c.data, "s": style})
        lines.append(runs)
    return lines


# Force a redraw at the real size.
os.kill(pid, 28)
pump(1.0)
dumps = {}
for step in steps:
    kind, _, arg = step.partition(":")
    if kind == "keys":
        os.write(fd, arg.encode())
        pump(0.5)
    elif kind == "sh":
        subprocess.run(arg, shell=True, check=True, cwd=ws)
        pump(0.9)
    elif kind == "wait":
        pump(int(arg) / 1000)
    elif kind == "dump":
        dumps[arg] = dump()
os.write(fd, b"q")
pump(0.3)
json.dump({"cols": cols, "rows": rows, "screens": dumps}, open(out, "w"), ensure_ascii=False)
