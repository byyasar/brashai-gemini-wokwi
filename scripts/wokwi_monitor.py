#!/usr/bin/env python3
"""
Wokwi RFC2217 Seri Port İzleyici (localhost:4000)
-------------------------------------------------
wokwi.toml içindeki `rfc2217ServerPort = 4000` üzerinden ESP32 seri port
çıktılarını terminalden canlı olarak takip eder (Telnet/RFC2217 IAC
kontrol baytlarını temizler).
"""

import socket
import sys
import time

HOST = "localhost"
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 4000
DURATION = float(sys.argv[2]) if len(sys.argv) > 2 else 0.0  # 0 = sınırsız


def strip_telnet_iac(data: bytes, state: list[int]) -> bytes:
    """Telnet/RFC2217 IAC (0xFF) kontrol dizilerini ayıklar."""
    out = bytearray()
    for b in data:
        st = state[0]
        if st == 0:  # Normal veri
            if b == 0xFF:
                state[0] = 1
            else:
                out.append(b)
        elif st == 1:  # IAC alındı
            if b == 0xFF:
                out.append(0xFF)
                state[0] = 0
            elif b == 0xFA:  # SB (subnegotiation)
                state[0] = 3
            elif b in (0xFB, 0xFC, 0xFD, 0xFE):  # WILL/WONT/DO/DONT + 1 bayt
                state[0] = 2
            else:
                state[0] = 0
        elif st == 2:  # 1 baytlık opsiyon atlandı
            state[0] = 0
        elif st == 3:  # SB içinde IAC bekle
            if b == 0xFF:
                state[0] = 4
        elif st == 4:  # SB içinde IAC sonrası SE (0xF0) bekle
            if b == 0xF0:
                state[0] = 0
            elif b != 0xFF:
                state[0] = 3
    return bytes(out)


def main() -> None:
    print(f"[Wokwi Monitor] {HOST}:{PORT} adresine bağlanılıyor...", flush=True)
    try:
        sock = socket.create_connection((HOST, PORT), timeout=2.0)
        sock.settimeout(2.0)
    except OSError as e:
        print(f"[HATA] {HOST}:{PORT} bağlantısı kurulamadı: {e}", file=sys.stderr)
        print("Wokwi simülasyonunu VS Code'da (yeniden) başlattığınızdan emin olun.", file=sys.stderr)
        sys.exit(1)

    print("[Wokwi Monitor] Bağlandı! Seri port çıktısı dinleniyor...\n", flush=True)
    state = [0]
    start = time.time()
    try:
        while True:
            if DURATION > 0 and (time.time() - start) >= DURATION:
                break
            try:
                chunk = sock.recv(4096)
                if not chunk:
                    print("\n[Wokwi Monitor] Bağlantı kapandı.", flush=True)
                    break
                clean = strip_telnet_iac(chunk, state)
                if clean:
                    sys.stdout.write(clean.decode("utf-8", errors="replace"))
                    sys.stdout.flush()
            except socket.timeout:
                continue
    except KeyboardInterrupt:
        print("\n[Wokwi Monitor] Durduruldu.")
    finally:
        sock.close()


if __name__ == "__main__":
    main()
