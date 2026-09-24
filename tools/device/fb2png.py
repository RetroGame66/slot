import sys
from PIL import Image

# fb2png.py <raw> <out.png> [w] [h] [bufidx] [order]
p = sys.argv[1]
out = sys.argv[2]
w = int(sys.argv[3]) if len(sys.argv) > 3 else 720
h = int(sys.argv[4]) if len(sys.argv) > 4 else 480
buf = int(sys.argv[5]) if len(sys.argv) > 5 else 0
order = sys.argv[6] if len(sys.argv) > 6 else "BGRA"

data = open(p, "rb").read()
need = w * h * 4
print(f"raw={len(data)} need={need} bufs={len(data)//need}")
start = buf * need
frame = data[start:start + need]
if len(frame) < need:
    print("not enough data for buffer", buf)
    sys.exit(1)

img = Image.frombytes("RGBA", (w, h), frame, "raw", order)
img.convert("RGB").save(out)
print("wrote", out)
