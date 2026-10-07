#!/usr/bin/env python3
"""Record the real dry-run TUI in a PTY and render its ANSI output to a GIF.

Requires Pillow and macOS Menlo/Hiragino fonts; no network or desktop capture.
"""
# SPDX-License-Identifier: MIT
import codecs
import json
import os
from pathlib import Path
import re
import select
import sys
import time
import unicodedata
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tests' / 'host'))
from test_dry_run import Terminal, Screen

COLORS = [(20, 25, 34), (226, 90, 103), (112, 207, 143), (230, 200, 117),
          (110, 160, 240), (190, 138, 219), (96, 210, 222), (222, 228, 237),
          (70, 78, 92), (246, 121, 133), (137, 231, 166), (249, 222, 142),
          (137, 189, 255), (212, 165, 242), (130, 234, 244), (249, 251, 255)]
BG, FG = COLORS[0], COLORS[7]


def palette(index):
    if index < 16:
        return COLORS[index]
    if index < 232:
        n = index - 16
        levels = [0, 95, 135, 175, 215, 255]
        return (levels[n // 36], levels[(n // 6) % 6], levels[n % 6])
    return (8 + 10 * (index - 232),) * 3


class StyledScreen(Screen):
    def __init__(self, cols=120, rows=24):
        self.pending = ''
        self.fg, self.bg = FG, BG
        super().__init__(cols, rows)

    def reset(self, cols, rows):
        super().reset(cols, rows)
        self.styles = [[(FG, BG)] * cols for _ in range(rows)]

    def sgr(self, args):
        codes = [int(n or 0) for n in args.split(';')]
        i = 0
        while i < len(codes):
            n = codes[i]
            if n == 0:
                self.fg, self.bg = FG, BG
            elif n == 39:
                self.fg = FG
            elif n == 49:
                self.bg = BG
            elif 30 <= n <= 37:
                self.fg = COLORS[n - 30]
            elif 40 <= n <= 47:
                self.bg = COLORS[n - 40]
            elif 90 <= n <= 97:
                self.fg = COLORS[n - 90 + 8]
            elif 100 <= n <= 107:
                self.bg = COLORS[n - 100 + 8]
            elif n in (38, 48) and i + 2 < len(codes):
                if codes[i + 1] == 5:
                    color = palette(codes[i + 2]); i += 2
                elif codes[i + 1] == 2 and i + 4 < len(codes):
                    color = tuple(codes[i + 2:i + 5]); i += 4
                else:
                    i += 1; continue
                if n == 38: self.fg = color
                else: self.bg = color
            i += 1

    def feed(self, text):
        self.pending += text
        while self.pending:
            if self.pending.startswith('\x1b'):
                if len(self.pending) < 2: return
                if self.pending[1] == '[':
                    match = re.match(r'\x1b\[([0-9;?]*)([A-Za-z~])', self.pending)
                    if not match: return
                    token = match.group(0)
                    if match.group(2) == 'm': self.sgr(match.group(1))
                else:
                    token = self.pending[:2]
                self.pending = self.pending[len(token):]
                super().feed(token)
            else:
                c, self.pending = self.pending[0], self.pending[1:]
                x, y = self.x, self.y
                if ord(c) >= 32 and 0 <= y < self.rows and 0 <= x < self.cols:
                    width = 2 if unicodedata.east_asian_width(c) in 'WF' else 1
                    for cell in range(x, min(x + width, self.cols)):
                        self.styles[y][cell] = (self.fg, self.bg)
                super().feed(c)


class Recording(Terminal):
    def __init__(self, binary):
        self.started = time.monotonic()
        self.events = []
        super().__init__(binary, ['tui', '--dry-run'], ROOT / 'artifacts/tui-recording/unused-state')
        self.screen = StyledScreen()
        self.resize(120, 24)

    def pump(self):
        if select.select([self.master], [], [], .05)[0]:
            output = self.decoder.decode(os.read(self.master, 65536))
            self.events.append([round(time.monotonic() - self.started, 4), 'o', output])
            self.screen.feed(output)

    def key(self, value):
        self.events.append([round(time.monotonic() - self.started, 4), 'i', value.decode()])
        super().key(value)


FONT = ImageFont.truetype('/System/Library/Fonts/Menlo.ttc', 15)
JP = ImageFont.truetype('/System/Library/Fonts/Hiragino Sans GB.ttc', 16)
CELL_W, CELL_H, PAD = 9, 23, 24


def render(screen, caption):
    image = Image.new('RGB', (screen.cols * CELL_W + PAD * 2, screen.rows * CELL_H + 118), BG)
    draw = ImageDraw.Draw(image)
    draw.text((PAD, 14), 'veil-warden  |  DRY-RUN tutorial', font=FONT, fill=COLORS[6])
    # Paint all cell backgrounds first so a wide glyph's right half is not erased.
    for y, row in enumerate(screen.cells):
        for x, c in enumerate(row):
            fg, bg = screen.styles[y][x]
            px, py = PAD + x * CELL_W, 50 + y * CELL_H
            draw.rectangle((px, py, px + CELL_W - 1, py + CELL_H - 1), fill=bg)
    for y, row in enumerate(screen.cells):
        for x, c in enumerate(row):
            if c and c != ' ':
                fg, _ = screen.styles[y][x]
                px, py = PAD + x * CELL_W, 50 + y * CELL_H
                font = JP if unicodedata.east_asian_width(c) in 'WF' else FONT
                draw.text((px, py + 1), c, font=font, fill=fg)
    draw.line((PAD, image.height - 54, image.width - PAD, image.height - 54), fill=COLORS[8])
    draw.text((PAD, image.height - 40), caption, font=JP, fill=COLORS[7])
    return image


def main():
    binary = sys.argv[1] if len(sys.argv) > 1 else str(Path.home() / '.local/bin/veil-warden')
    # Preserve the terminal's normal colors rather than Codex's log-oriented override.
    os.environ.pop('NO_COLOR', None)
    recording = Recording(binary)
    frames, durations = [], []
    def capture(caption, duration):
        frames.append(render(recording.screen, caption)); durations.append(duration)
    try:
        text = recording.screen.text
        recording.wait(lambda: 'DRY-RUN' in text() and '203.0.113.30:8443' in text())
        capture('1. 架空の接続試行を表示。実通信・実拒否は行いません。', 1600)
        recording.key(b'b'); recording.wait(lambda: '確認:' in text())
        capture('2. b：選択した宛先を確認。Enterを押すまで変更しません。', 1300)
        recording.key(b'\r'); recording.wait(lambda: '模擬ルールを追加' in text())
        capture('3. Enter：模擬ルールを登録。次の接続から適用します。', 1100)
        recording.key(b'n'); recording.wait(lambda: '試行=4 拒否=1' in text())
        capture('4. n：再試行すると、新しいイベントが DENY になります。', 1900)
        recording.key(b'\td'); recording.wait(lambda: '解除する' in text())
        capture('5. Tab → d：解除する宛先を確認します。', 1100)
        recording.key(b'\r'); recording.wait(lambda: '模擬ルールを解除' in text())
        recording.key(b'n'); recording.wait(lambda: '試行=5 拒否=1' in text())
        capture('6. Enter → n：解除後の再試行は ALLOW に戻ります。', 1900)
        recording.key(b'q'); recording.wait(lambda: 'PREVIEW_EXIT=0' in text())
        assert not recording.screen.alternate
        recording.process.wait(timeout=5)
    finally:
        recording.close()
    output = ROOT / 'docs/images/dry-run-tui.gif'
    if output.exists(): raise RuntimeError('Output exists; preserve it before recording again')
    frames[0].save(output, save_all=True, append_images=frames[1:], duration=durations, loop=0, optimize=True, disposal=2)
    cast = ROOT / 'artifacts/tui-recording/dry-run.cast'
    header = {'version': 2, 'width': 120, 'height': 24, 'title': 'veil-warden dry-run tutorial'}
    # The cast keeps original output timing; GIF holds each key state for reading.
    cast.write_text(json.dumps(header) + '\n' + '\n'.join(json.dumps(event, ensure_ascii=False) for event in recording.events) + '\n')
    frames[3].save(ROOT / 'artifacts/tui-recording/denied.png')
    frames[-1].save(ROOT / 'artifacts/tui-recording/restored.png')
    print(json.dumps({'gif': str(output), 'frames': len(frames), 'duration_ms': sum(durations), 'bytes': output.stat().st_size, 'cast': str(cast)}))


if __name__ == '__main__': main()
