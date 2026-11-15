#include "Arduino.h"
#include "FS.h"
#include "SPI.h"
#include <vs1053_ext.h>
#include <vs1053b-patches-flac.h>

#define SD_CS 5
#define VS1053_MOSI 23
#define VS1053_MISO 19
#define VS1053_SCK 18
#define VS1053_CS 32
#define VS1053_DCS 33
#define VS1053_DREQ 35

int volume = 100;
bool isPlaying = false;
String path = "";
volatile bool sarkiDurduruluyor = false;
volatile unsigned long sonSarkiBaslamaZamani = 0;
volatile bool sonrakiSarkiyaGec = false;
volatile bool efektCalIstegi = false;
volatile bool efektCaliniyor = false;
String calinacakEfektDosyasi = "";

VS1053 mp3(VS1053_CS, VS1053_DCS, VS1053_DREQ, VSPI, VS1053_MOSI, VS1053_MISO,
           VS1053_SCK);

void MuzikDurdurFnc() {
  sarkiDurduruluyor = true;
  sonrakiSarkiyaGec = false;
  if (mp3.isRunning()) {
    mp3.stop_mp3client();
  }
  isPlaying = false;
  delay(100);
  sarkiDurduruluyor = false;
}

void playRandomSongFromFolder(const char *folderPath) {
  if (mp3.isRunning()) {
    mp3.stop_mp3client(); // Sadece çalışıyorsa durdur
  }
  Serial.print("[INFO] Opening folder: ");
  Serial.println(folderPath);

  File root = SD.open(folderPath);
  if (!root || !root.isDirectory()) {
    Serial.println("[WARN] Folder open failed! SD kart yeniden baslatiliyor...");
    SD.end();
    delay(50);
    SPI.begin(VS1053_SCK, VS1053_MISO, VS1053_MOSI);
    SD.begin(SD_CS, SPI, 1000000);
    delay(50);
    root = SD.open(folderPath);
    if (!root || !root.isDirectory()) {
      Serial.println("[ERROR] Folder open failed definitively!");
      return;
    }
  }

  // Tüm dosyaları say
  int fileCount = 0;
  File file = root.openNextFile();
  while (file) {
    if (!file.isDirectory()) {
      fileCount++;
      Serial.print("[INFO] Found file: ");
      Serial.println(file.name());
    }
    file.close(); // <-- Eklendi
    file = root.openNextFile();
  }

  root.close();               // Close directory instead of rewinding
  root = SD.open(folderPath); // Reopen directory
  if (!root) {
    Serial.println("[ERROR] Failed to reopen folder!");
    return;
  }

  if (fileCount == 0) {
    Serial.println("[ERROR] No files in folder!");
    root.close();
    return;
  }

  int targetIndex = random(fileCount);
  Serial.print("[INFO] Randomly selected file index: ");
  Serial.println(targetIndex);

  int currentIndex = 0;
  String secilenDosya = "";
  file = root.openNextFile();
  while (file) {
    if (!file.isDirectory()) {
      if (currentIndex == targetIndex) {
        secilenDosya = String(file.name());
        file.close();
        break;
      }
      currentIndex++;
    }
    file.close();
    file = root.openNextFile();
  }

  // Çalmadan önce tüm dosya/klasör tutamaklarını kapat (aynı dosya iki kez açık kalmasın)
  root.close();

  if (secilenDosya == "") {
    Serial.println("[ERROR] Dosya secilemedi!");
    return;
  }

  path = String(folderPath);
  path.concat("/");
  path.concat(secilenDosya);
  Serial.print("[INFO] Now playing: ");
  Serial.println(path);

  Serial.print("[DEBUG] Free Heap: ");
  Serial.print(ESP.getFreeHeap());
  Serial.print(" | Max Alloc Block: ");
  Serial.println(ESP.getMaxAllocHeap());

  // Daha önce şarkı çalındıysa VS1053'ü temiz duruma getir
  static bool ilkCalma = true;
  if (!ilkCalma) {
    mp3.begin();
    delay(50);
  }
  ilkCalma = false;

  mp3.setVolume(volume);
  bool durum = mp3.connecttoFS(SD, path.c_str());
  if (!durum) {
    Serial.println("[WARN] connecttoFS ilk deneme basarisiz, tekrar deneniyor...");
    mp3.begin();
    mp3.setVolume(volume);
    durum = mp3.connecttoFS(SD, path.c_str());
  }

  if (durum) {
    Serial.print("[INFO] Playback started | isRunning: ");
    Serial.println(mp3.isRunning() ? "EVET" : "HAYIR");
    isPlaying = true;
    sonSarkiBaslamaZamani = millis();
    sonrakiSarkiyaGec = false;
  } else {
    Serial.println("[ERROR] Playback failed definitively");
  }
}

void playSongFile(const char *filePath) {
  if (mp3.isRunning()) {
    mp3.stop_mp3client();
  }
  Serial.print("[INFO] Playing specific file: ");
  Serial.println(filePath);

  path = String(filePath);

  static bool ilkCalma = true;
  if (!ilkCalma) {
    mp3.begin();
    delay(50);
  }
  ilkCalma = false;

  mp3.setVolume(volume);
  bool durum = mp3.connecttoFS(SD, path.c_str());
  if (!durum) {
    Serial.println("[WARN] connecttoFS dosya calinamadi, tekrar deneniyor...");
    mp3.begin();
    mp3.setVolume(volume);
    durum = mp3.connecttoFS(SD, path.c_str());
  }

  if (durum) {
    Serial.print("[INFO] File playback started | isRunning: ");
    Serial.println(mp3.isRunning() ? "EVET" : "HAYIR");
    isPlaying = true;
    sonSarkiBaslamaZamani = millis();
    sonrakiSarkiyaGec = false;
  } else {
    Serial.println("[ERROR] File playback failed definitively");
  }
}
