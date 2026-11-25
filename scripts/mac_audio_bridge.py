#!/usr/bin/env python3
"""
BrashAI Wokwi -> macOS Gerçek Stereo Ses Köprüsü (scripts/mac_audio_bridge.py)
------------------------------------------------------------------------------
Wokwi simülatörünün RFC2217 seri portuna (localhost:4000) bağlanır.
ESP32'nin VS1053'e çaldırdığı SD kart MP3 dosyalarını anlık algılar ve
macOS'un yerleşik `afplay` motoru ile doğrudan Mac hoparlörünüzden
gerçek stereo kalitesinde çalar.

Ekstra paket kurulumu gerektirmez (yalnızca Python 3 standart kütüphanesi).

Kullanım:
  python3 scripts/mac_audio_bridge.py           # Wokwi'ye bağlan ve dinle
  python3 scripts/mac_audio_bridge.py --test    # Mac hoparlörünü tada.mp3 ile anında test et
  python3 scripts/mac_audio_bridge.py --port 4000 --volume 1.0
"""

import argparse
import os
import re
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
SDCARD_DIR = ROOT_DIR / "sdcard"
AFPLAY_BIN = "/usr/bin/afplay"

# ESP32 Serial çıktısından yakalanacak MP3 çalma / durdurma / ses seviyesi kalıpları
PLAY_PATTERNS = [
    re.compile(r"\[AUDIO_PLAY\]\s*(/[^\r\n]+)"),
    re.compile(r"\[MP3\]\s*Caliniyor\s*->\s*(/[^\r\n]+)"),
    re.compile(r"\[INFO\]\s*Playing specific file:\s*(/[^\r\n]+)"),
    re.compile(r"\[INFO\]\s*Now playing:\s*(/[^\r\n]+)"),
]

STOP_PATTERNS = [
    re.compile(r"\[AUDIO_STOP\]"),
    re.compile(r"Müzik.*durduruluyor", re.IGNORECASE),
    re.compile(r"Muzik.*durduruluyor", re.IGNORECASE),
]

VOL_PATTERNS = [
    re.compile(r"\[AUDIO_VOL\]\s*(\d+)"),
    re.compile(r"Volume:\s*(\d+)\s*/\s*(\d+)"),
]


class MacAudioPlayer:
    def __init__(self, volume: float = 1.0) -> None:
        self.volume = max(0.0, min(2.0, volume))
        self.proc: subprocess.Popen | None = None
        self.current_file: str | None = None

    def resolve_sd_path(self, sd_path: str) -> Path | None:
        clean = sd_path.strip().lstrip("/")
        direct = SDCARD_DIR / clean
        if direct.is_file():
            return direct

        # Büyük/küçük harf duyarsız arama (FAT dosya sistemi uyumluluğu)
        parts = clean.split("/")
        cur = SDCARD_DIR
        for part in parts:
            if not cur.is_dir():
                return None
            match = None
            for child in cur.iterdir():
                if child.name.lower() == part.lower():
                    match = child
                    break
            if match is None:
                return None
            cur = match
        return cur if cur.is_file() else None

    def stop(self) -> None:
        if self.proc is not None:
            if self.proc.poll() is None:
                try:
                    self.proc.terminate()
                    self.proc.wait(timeout=0.5)
                except Exception:
                    try:
                        self.proc.kill()
                    except Exception:
                        pass
            self.proc = None
            self.current_file = None

    def play(self, sd_path: str) -> None:
        local_file = self.resolve_sd_path(sd_path)
        if local_file is None:
            print(f"\033[33m[KOPRU UYARI] Dosya sdcard/ icinde bulunamadi: {sd_path}\033[0m")
            return

        self.stop()
        cmd = [AFPLAY_BIN, "-v", f"{self.volume:.2f}", str(local_file)]
        try:
            self.proc = subprocess.Popen(
                cmd,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            self.current_file = str(local_file.relative_to(ROOT_DIR))
            print(
                f"\033[92m[KOPRU 🔊] CALIYOR: {self.current_file} "
                f"(Ses: {int(self.volume * 100)}%)\033[0m"
            )
        except FileNotFoundError:
            print(f"\033[91m[KOPRU HATA] {AFPLAY_BIN} bulunamadi!\033[0m")
        except Exception as exc:
            print(f"\033[91m[KOPRU HATA] Ses baslatilamadi: {exc}\033[0m")

    def set_volume_ratio(self, ratio: float) -> None:
        self.volume = max(0.0, min(1.5, ratio))


class Rfc2217Cleaner:
    """Telnet / RFC2217 IAC (0xFF) kontrol dizilerini temizleyerek saf seri metin çıkarır."""

    def __init__(self) -> None:
        self.state = 0  # 0=normal, 1=IAC, 2=cmd_arg, 3=SB, 4=SB_IAC

    def feed(self, data: bytes) -> bytes:
        out = bytearray()
        for b in data:
            if self.state == 0:
                if b == 0xFF:
                    self.state = 1
                else:
                    out.append(b)
            elif self.state == 1:
                if b == 0xFF:
                    out.append(0xFF)
                    self.state = 0
                elif b == 0xFA:  # SB (Subnegotiation)
                    self.state = 3
                elif b in (0xFB, 0xFC, 0xFD, 0xFE):  # WILL/WONT/DO/DONT
                    self.state = 2
                else:
                    self.state = 0
            elif self.state == 2:
                self.state = 0
            elif self.state == 3:
                if b == 0xFF:
                    self.state = 4
            elif self.state == 4:
                if b == 0xF0:  # SE (Subnegotiation End)
                    self.state = 0
                elif b != 0xFF:
                    self.state = 3
        return bytes(out)


def handle_serial_line(line: str, player: MacAudioPlayer) -> None:
    print(f"[ESP32] {line}")

    for pat in VOL_PATTERNS:
        m = pat.search(line)
        if m:
            if len(m.groups()) == 2:
                val, max_v = int(m.group(1)), max(1, int(m.group(2)))
                player.set_volume_ratio(val / max_v)
            else:
                val = int(m.group(1))
                player.set_volume_ratio(val / 21.0 if val <= 21 else val / 100.0)
            break

    for pat in STOP_PATTERNS:
        if pat.search(line):
            player.stop()
            print("\033[93m[KOPRU ⏹] Ses durduruldu.\033[0m")
            return

    for pat in PLAY_PATTERNS:
        m = pat.search(line)
        if m:
            sd_file = m.group(1).strip()
            player.play(sd_file)
            return


def run_bridge(host: str, port: int, volume: float) -> None:
    player = MacAudioPlayer(volume=volume)

    def _cleanup(_sig, _frame):
        print("\n[KOPRU] Kapatiliyor...")
        player.stop()
        sys.exit(0)

    signal.signal(signal.SIGINT, _cleanup)
    signal.signal(signal.SIGTERM, _cleanup)

    print("============================================================")
    print("  BrashAI Wokwi -> macOS Gercek Stereo Ses Koprusu")
    print(f"  Hedef : {host}:{port} (wokwi.toml rfc2217ServerPort)")
    print(f"  SD    : {SDCARD_DIR}")
    print("  Cikis : Ctrl+C")
    print("============================================================")

    waiting_printed = False
    while True:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(3.0)
        try:
            sock.connect((host, port))
            sock.settimeout(1.0)
            waiting_printed = False
            print(f"\033[96m[KOPRU] Wokwi seri portuna baglandi ({host}:{port})! Dinleniyor...\033[0m")

            cleaner = Rfc2217Cleaner()
            buf = ""
            while True:
                try:
                    chunk = sock.recv(4096)
                    if not chunk:
                        print("\033[33m[KOPRU] Wokwi baglantisi kapandi. Yeniden bekleniyor...\033[0m")
                        player.stop()
                        break
                    clean_bytes = cleaner.feed(chunk)
                    if not clean_bytes:
                        continue
                    buf += clean_bytes.decode("utf-8", errors="replace")
                    while "\n" in buf:
                        line, buf = buf.split("\n", 1)
                        line = line.rstrip("\r").strip()
                        if line:
                            handle_serial_line(line, player)
                except socket.timeout:
                    continue
        except (ConnectionRefusedError, OSError):
            if not waiting_printed:
                print(
                    f"[KOPRU] Wokwi simulasyonu bekleniyor ({host}:{port})... "
                    "VS Code'da Wokwi'yi baslattiginizda otomatik baglanacak."
                )
                waiting_printed = True
            time.sleep(1.0)
        finally:
            try:
                sock.close()
            except Exception:
                pass


def main() -> None:
    parser = argparse.ArgumentParser(description="Wokwi -> macOS Gerçek Stereo MP3 Ses Köprüsü")
    parser.add_argument("--host", default="127.0.0.1", help="Wokwi RFC2217 adresi (varsayılan: 127.0.0.1)")
    parser.add_argument("--port", type=int, default=4000, help="Wokwi RFC2217 portu (varsayılan: 4000)")
    parser.add_argument("--volume", type=float, default=1.0, help="Başlangıç ses seviyesi 0.0..1.5 (varsayılan: 1.0)")
    parser.add_argument("--test", action="store_true", help="Mac hoparlörünü hemen /efekt/tada.mp3 ile test et")
    args = parser.parse_args()

    if args.test:
        player = MacAudioPlayer(volume=args.volume)
        print("[KOPRU TEST] Mac hoparlorunden /efekt/tada.mp3 caliniyor...")
        player.play("/efekt/tada.mp3")
        if player.proc is not None:
            player.proc.wait()
        print("[KOPRU TEST] Tamamlandi!")
        return

    run_bridge(args.host, args.port, args.volume)


if __name__ == "__main__":
    main()
