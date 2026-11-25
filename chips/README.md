# BrashAI Wokwi Geliştirme Kartları (VS1053 + SD Kart + Hoparlör)

Bu dizin, [VLSI VS1053b](https://cdn-shop.adafruit.com/datasheets/vs1053.pdf) ses kodek çipi için Wokwi Custom Chips API (Rust → WebAssembly) kullanılarak geliştirilmiş üç donanım modülünü içerir:

## 1. Modüller ve Pin Bağlantıları

### A. VS1053b Ses Kodek Geliştirme Kartı (`chip-vs1053`)
* **SPI & Kontrol Pinleri (ESP32 `src/MuzikIslemleri.h` ile birebir):**
  * `SCK` → `GPIO 18`
  * `MOSI` → `GPIO 23`
  * `MISO` → `GPIO 19`
  * `XCS` (SCI Chip Select) → `GPIO 32`
  * `XDCS` (SDI Data Chip Select) → `GPIO 33`
  * `DREQ` (Data Request el sıkışması) → `GPIO 35`
  * `XRST` → `VCC` (3.3V)
* **Ses Çıkışları:**
  * `SPK_OUT`: 32 kHz Sigma-Delta (PDM) dijital ses çıkışı (`chip-speaker:SPK_IN` üzerinden `wokwi-buzzer` `accurate` moduna bağlı).
  * `LEFT` / `RIGHT`: Sol ve sağ kanal zarf voltajı (`0.0V..3.3V`, VU-metre ve diyafram için).
* **Dahili Ekran ve Kontroller:**
  * `160x96` canlı durum ekranı: Aktif format (`MP3`/`WAV`/`TEST`), örnekleme hızı, kanal (`MO`/`ST`), bitrate, `SCI_VOL` ses yüzdesi, `SCI_DECODE_TIME`, 2048B SDI FIFO doluluk çubuğu ve `P` (Güç) / `D` (`DREQ`) / Çalma LED'leri.
  * **Çıkış Kazancı (%):** Wokwi arayüzünden `10%..200%` arası ayarlanabilir.

### B. SD Kart Geliştirme Kartı (`wokwi-microsd-card` + `chip-sd-board`)
* Gerçek FAT16/32 dosya sistemi okumaları `wokwi.toml` içindeki `[[sdcard]] folder = "sdcard"` üzerinden yerleşik `wokwi-microsd-card` modülüyle yapılır (`CS = GPIO 5`).
* `chip-sd-board` aynı SPI hattını salt-okunur (`INPUT`) modda izler; `128x80` ekranında `PWR`/`ACT` LED'lerini, `CD` kart durumunu, `CS` aktifliğini, toplam aktarılan bayt/KB miktarını ve anlık hız çubuğunu gösterir.

### C. Hoparlör Geliştirme Kartı (`chip-speaker` + `wokwi-buzzer`)
* `chip-speaker`, `SPK_IN` üzerindeki Sigma-Delta PDM işaretini gecikmesiz olarak `SPK_OUT` üzerinden `wokwi-buzzer` (`mode: "accurate"`) bileşenine iletir; böylece Wokwi tarayıcısında gerçek ses duyulur.
* `128x128` ekranında ses genliğine göre canlı titreşen hoparlör diyaframı, akustik dalga halkaları, stereo `L/R` VU-metre ve `Sessize Al (0/1)` anahtarı yer alır.

---

## 2. SD Kart Klasör Yapısı (`sdcard/`)
`src/main.ino` ve `src/MuzikIslemleri.h` aşağıdaki yolları okur:
* `sdcard/hosgeldiniz/Firca1.mp3` .. `Firca5.mp3`
* `sdcard/1/sarki1.mp3` .. `sdcard/5/sarki1.mp3`
* `sdcard/efekt/tada.mp3`

Kendi `.mp3` dosyalarınızı `sdcard/` altındaki bu klasörlere koyabilirsiniz (yeniden derleme gerekmez). Test seslerini sıfırdan üretmek için:
```bash
python3 scripts/generate_sdcard_audio.py
```

---

## 3. Derleme ve Test Komutları

### Birim Testleri (`cargo test`)
```bash
cargo test --manifest-path chips/Cargo.toml
```

### WASM Çiplerini Yeniden Derleme
```bash
./chips/build.sh
```
Bu komut `chips/vs1053.chip.wasm`, `chips/sd-board.chip.wasm` ve `chips/speaker.chip.wasm` dosyalarını üretir.

### Wokwi'de Çalıştırma
1. PlatformIO ile ESP32 firmware'ini derleyin (`.pio/build/esp32doit-devkit-v1/firmware.bin` ve `.elf` oluşsun).
2. VS Code'da Wokwi eklentisiyle `diagram.json` dosyasını açıp simülasyonu başlatın (**tarayıcı/eklenti sesinin açık olduğundan emin olun**).
