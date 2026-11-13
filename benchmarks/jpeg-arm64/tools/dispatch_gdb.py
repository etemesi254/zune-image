import json
import os
import subprocess

import gdb


def group(name):
    if name.startswith("zune_jpeg::color_convert::neon"):
        return "neon_color"
    if name.startswith("zune_jpeg::color_convert::scalar::ycbcr_to_"):
        return "scalar_color"
    for architecture in ("neon", "scalar"):
        prefix = f"zune_jpeg::upsampler::{architecture}::upsample_"
        if name.startswith(prefix):
            for kind in ("horizontal", "vertical", "hv"):
                if name.startswith(prefix + kind):
                    return f"{architecture}_{kind}"
        if name.startswith(f"zune_jpeg::idct::{architecture}::idct"):
            return f"{architecture}_idct"
    return None


COUNTS = {}
LIMIT = 32


class Entry(gdb.Breakpoint):
    def __init__(self, name, category):
        super().__init__("'" + name + "'", internal=True)
        self.function = name
        self.category = category
        self.hits = 0

    def stop(self):
        self.hits += 1
        COUNTS[self.function] = {"group": self.category, "capped_entry_hits": self.hits}
        if self.hits == LIMIT:
            self.enabled = False
        return False


gdb.execute("set pagination off")
gdb.execute("set confirm off")
gdb.execute("set disable-randomization off")
symbols = subprocess.check_output(["nm", "-C", "--defined-only", os.environ["JPEG_PROBE_BINARY"]], text=True)
entries = []
seen = set()
for line in symbols.splitlines():
    fields = line.split(None, 2)
    if len(fields) != 3 or fields[1] not in ("T", "t"):
        continue
    name = fields[2]
    category = group(name)
    if category and name not in seen and "closure" not in name:
        entries.append(Entry(name, category))
        seen.add(name)
if not entries:
    raise RuntimeError("no implementation symbols resolved; dispatch is unverified")
gdb.execute("run")
exit_code = gdb.parse_and_eval("$_exitcode")
if int(exit_code) != 0:
    raise RuntimeError(f"dispatch workload failed: {exit_code}")
with open(os.environ["JPEG_PROBE_OUTPUT"], "w") as output:
    json.dump({"functions": COUNTS, "entry_limit_per_function": LIMIT,
               "method": "named implementation-entry breakpoints; no function-pointer equality"}, output, indent=2)
