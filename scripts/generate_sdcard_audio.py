#!/usr/bin/env python3
"""
BrashAI Wokwi SD Kart Test Ses Dosyaları Üretici
------------------------------------------------
src/main.ino ve src/MuzikIslemleri.h dosyalarının beklediği
/hosgeldiniz/Firca1..5.mp3, /1..5/sarki1.mp3 ve /efekt/tada.mp3
dosyalarını yumuşak zarflı gerçek polifonik sentez + ffmpeg (libmp3lame)
ile 22050 Hz, mono, 32 kbps CBR MP3 olarak üretir.
"""

import math
import os
import struct
import subprocess
import sys
import tempfile
import wave

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SDCARD_DIR = os.path.join(ROOT, "sdcard")
FFMPEG = "/opt/homebrew/bin/ffmpeg"
if not os.path.exists(FFMPEG):
    FFMPEG = "ffmpeg"

SAMPLE_RATE = 22050


def synth_notes_to_mp3(notes: list[tuple[float, float, float]], out_path: str) -> None:
    """
    notes: [(frekans_hz, sure_sn, harmonik_karisim), ...]
    Tıklamayı önlemek için her notaya kısa attack/release zarfı uygular.
    """
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    samples: list[int] = []

    for freq, dur, harmonic in notes:
        total = int(SAMPLE_RATE * dur)
        attack = max(1, int(SAMPLE_RATE * 0.015))
        release = max(1, int(SAMPLE_RATE * 0.04))
        for i in range(total):
            t = i / SAMPLE_RATE
            env = 1.0
            if i < attack:
                env = i / attack
            elif i > total - release:
                env = max(0.0, (total - i) / release)
            # Temel frekans + hafif 2. harmonik zenginliği
            val = (
                0.70 * math.sin(2.0 * math.pi * freq * t)
                + harmonic * math.sin(2.0 * math.pi * (freq * 2.0) * t)
            )
            pcm = int(max(-32000, min(32000, val * env * 22000.0)))
            samples.append(pcm)

    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
        tmp_wav = tmp.name

    try:
        with wave.open(tmp_wav, "wb") as wf:
            wf.setnchannels(1)
            wf.setsampwidth(2)
            wf.setframerate(SAMPLE_RATE)
            wf.writeframes(struct.pack(f"<{len(samples)}h", *samples))

        cmd = [
            FFMPEG,
            "-y",
            "-i",
            tmp_wav,
            "-ac",
            "1",
            "-ar",
            str(SAMPLE_RATE),
            "-b:a",
            "32k",
            "-codec:a",
            "libmp3lame",
            "-write_xing",
            "0",
            out_path,
        ]
        res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        if res.returncode != 0:
            print(f"[HATA] {out_path} üretilemedi:\n{res.stderr}", file=sys.stderr)
            sys.exit(1)
        size_kb = os.path.getsize(out_path) / 1024.0
        print(f"[OK] {os.path.relpath(out_path, ROOT)} ({size_kb:.1f} KB)")
    finally:
        if os.path.exists(tmp_wav):
            os.remove(tmp_wav)


def main() -> None:
    # 1. Hoşgeldiniz anons arpejleri (/hosgeldiniz/Firca1..5.mp3)
    firca_arpejleri = [
        [(523.25, 0.45, 0.15), (659.25, 0.45, 0.15), (783.99, 0.80, 0.15)],   # Firca 1: Do majör
        [(587.33, 0.45, 0.15), (739.99, 0.45, 0.15), (880.00, 0.80, 0.15)],   # Firca 2: Re majör
        [(659.25, 0.45, 0.15), (830.61, 0.45, 0.15), (987.77, 0.80, 0.15)],   # Firca 3: Mi majör
        [(698.46, 0.45, 0.15), (880.00, 0.45, 0.15), (1046.50, 0.80, 0.15)],  # Firca 4: Fa majör
        [(783.99, 0.45, 0.15), (987.77, 0.45, 0.15), (1174.66, 0.80, 0.15)],  # Firca 5: Sol majör
    ]
    for idx, notes in enumerate(firca_arpejleri, start=1):
        synth_notes_to_mp3(notes, os.path.join(SDCARD_DIR, "hosgeldiniz", f"Firca{idx}.mp3"))

    # 2. Fırça klasörü müzikleri (/1..5/sarki1.mp3) - ~4.5 sn neşeli melodiler
    melodies = [
        [(523.25, 0.45, 0.2), (587.33, 0.45, 0.2), (659.25, 0.45, 0.2), (783.99, 0.65, 0.2),
         (659.25, 0.45, 0.2), (587.33, 0.45, 0.2), (523.25, 0.80, 0.2), (659.25, 0.70, 0.2)],
        [(392.00, 0.45, 0.2), (440.00, 0.45, 0.2), (493.88, 0.45, 0.2), (587.33, 0.65, 0.2),
         (493.88, 0.45, 0.2), (440.00, 0.45, 0.2), (392.00, 0.80, 0.2), (587.33, 0.70, 0.2)],
        [(440.00, 0.45, 0.2), (523.25, 0.45, 0.2), (659.25, 0.45, 0.2), (880.00, 0.65, 0.2),
         (783.99, 0.45, 0.2), (659.25, 0.45, 0.2), (523.25, 0.80, 0.2), (440.00, 0.70, 0.2)],
        [(349.23, 0.45, 0.2), (440.00, 0.45, 0.2), (523.25, 0.45, 0.2), (698.46, 0.65, 0.2),
         (587.33, 0.45, 0.2), (523.25, 0.45, 0.2), (440.00, 0.80, 0.2), (349.23, 0.70, 0.2)],
        [(329.63, 0.45, 0.2), (392.00, 0.45, 0.2), (493.88, 0.45, 0.2), (659.25, 0.65, 0.2),
         (587.33, 0.45, 0.2), (493.88, 0.45, 0.2), (392.00, 0.80, 0.2), (329.63, 0.70, 0.2)],
    ]
    for idx, notes in enumerate(melodies, start=1):
        synth_notes_to_mp3(notes, os.path.join(SDCARD_DIR, str(idx), "sarki1.mp3"))

    # 3. Başarı efekti (/efekt/tada.mp3) - Yükselen C5-E5-G5-C6 fanfar (1.8 sn)
    tada_notes = [
        (523.25, 0.25, 0.25),
        (659.25, 0.25, 0.25),
        (783.99, 0.25, 0.25),
        (1046.50, 1.05, 0.20),
    ]
    synth_notes_to_mp3(tada_notes, os.path.join(SDCARD_DIR, "efekt", "tada.mp3"))


if __name__ == "__main__":
    main()
