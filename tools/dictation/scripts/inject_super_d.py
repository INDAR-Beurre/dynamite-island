#!/usr/bin/env python3
"""Inject Super+D (and inert legacy chords) from a temporary uinput keyboard.

The injected node appears before the app's 2 s /dev/input poll, so the app arms it.
Usage: python3 scripts/inject_super_d.py [chord|old|plain]
"""
import fcntl
import glob
import os
import struct
import sys
import time

UI_SET_EVBIT = 0x40045564
UI_SET_KEYBIT = 0x40045565
UI_DEV_CREATE = 0x5501
UI_DEV_DESTROY = 0x5502
EV_KEY = 0x01
KEYS = [30, 32, 38, 56, 100, 125, 126]  # A, D, L, left/right Alt, left/right Meta

SEQUENCES = {
    "chord": [(125, 1), (32, 1), (32, 0), (125, 0)],
    "old": [(100, 1), (38, 1), (38, 0), (100, 0), (56, 1), (30, 1), (30, 0), (56, 0)],
    "plain": [(32, 1), (32, 0)],
}


def event(code, value):
    t = time.time()
    return struct.pack("llHHi", int(t), int((t % 1) * 1e6), EV_KEY, code, value)


def syn():
    t = time.time()
    return struct.pack("llHHi", int(t), int((t % 1) * 1e6), 0, 0, 0)


mode = sys.argv[1] if len(sys.argv) > 1 else "chord"
fd = os.open("/dev/uinput", os.O_WRONLY)
fcntl.ioctl(fd, UI_SET_EVBIT, EV_KEY)
for code in KEYS:
    fcntl.ioctl(fd, UI_SET_KEYBIT, code)
dev = struct.pack("<80sHHHHI64l64l64l64l", b"dictation-test-kbd", 0x03, 1, 1, 1, 0,
                  *([0] * 64), *([0] * 64), *([0] * 64), *([0] * 64))
os.write(fd, dev)
fcntl.ioctl(fd, UI_DEV_CREATE)
print("injected node:", ", ".join(sorted(glob.glob("/dev/input/event*"))[-3:]), flush=True)
time.sleep(3)  # let the app's 2 s poll discover the new node
for code, value in SEQUENCES[mode]:
    os.write(fd, event(code, value) + syn())
    time.sleep(0.25)
time.sleep(1)
fcntl.ioctl(fd, UI_DEV_DESTROY)
os.close(fd)
print("injected", mode, "sequence", flush=True)
