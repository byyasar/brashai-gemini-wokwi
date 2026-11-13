
#include <Adafruit_NeoPixel.h>
#include <LedDisplayIslemleri.h>
#include "WifiIslemleri.h"
#include "SupabaseIslemleri.h"
#include <MuzikIslemleri.h>
#include "soc/soc.h"
#include "soc/rtc_cntl_reg.h"

TaskHandle_t TaskCore0;
TaskHandle_t TaskCore1;

#define BTNPLAY_PIN 22
#define BTNSTOP_PIN 39
#define BTNREFRESH_PIN 36
#define FIRCA1_PIN 25
#define FIRCA2_PIN 4
#define FIRCA3_PIN 34
#define FIRCA4_PIN 26
#define FIRCA5_PIN 12
#define ORTAMLDR_PIN 27
#define LED_COUNT 9
#define FAN_PIN 2
#define KAPAK_PIN 21
#define SERITLED_PIN 17
#define KAPAK_MOSFET_PIN 16 // Kapak gerilimi MOSFET sürücü pini

// Kapak Sensörü Mantığı (Kapak açıkken 1 / HIGH, kapalıyken 0 / LOW)
#define KAPAK_ACIK_SEVIYE 1

u8_t b1 = 0;
u8_t b2 = 0;
u8_t b3 = 0;
u8_t kapak = 0;
u16_t firca1Ldr = 0;
u16_t firca2Ldr = 0;
u16_t firca3Ldr = 0;
u16_t firca4Ldr = 0;
u16_t firca5Ldr = 0;
u16_t ortamLdr = 0;

u16_t LdrEsikDeger = 2000;

Adafruit_NeoPixel strip(LED_COUNT, SERITLED_PIN, NEO_GRB + NEO_KHZ800);

String klasor = "";
String fircaId = "";
String cihazId = "Cihaz1";

void setup() {
  WRITE_PERI_REG(RTC_CNTL_BROWN_OUT_REG, 0); // Düşük voltaj/brownout resetini engelle
  Serial.begin(115200);
  delay(300);
  Serial.println("\n[BrashAI] Sistem baslatiliyor...");

  // Donanım pinleri
  pinMode(BTNPLAY_PIN, INPUT);
  pinMode(BTNSTOP_PIN, INPUT);
  pinMode(BTNREFRESH_PIN, INPUT);
  pinMode(FIRCA1_PIN, INPUT);
  pinMode(FIRCA2_PIN, INPUT);
  pinMode(FIRCA3_PIN, INPUT);
  pinMode(FIRCA4_PIN, INPUT);
  pinMode(FIRCA5_PIN, INPUT);
  pinMode(ORTAMLDR_PIN, INPUT);
  pinMode(FAN_PIN, OUTPUT);
  pinMode(KAPAK_PIN, INPUT_PULLUP); // Dahili Pull-Up ile kararlı kapak okuma

  // NeoPixel Şerit LED Başlatma
  strip.begin();
  strip.setBrightness(150);
  strip.show(); // Başlangıçta tüm LED'leri söndür

  /*Kum saati baslangıcı*/
  for (byte i = 0; i < 2; i++) {
    lc.shutdown(i, false);
    lc.setIntensity(i, 2);
  }
  resetTime();
  /*Kum saati bitis*/

  // İlk açılışta Wi-Fi kontrolü (Bağlanamazsa AP moduna geçer)
  WifiIlkAcilisKontrol();

  SPI.begin(VS1053_SCK, VS1053_MISO, VS1053_MOSI);
  if (!SD.begin(SD_CS, SPI, 1000000)) { // 1 MHz SPI hızı
    // Serial.println("SD Card initialization failed!");
    while (!SD.begin(SD_CS))
      ;
  }

  // SD kart işlemleri tamamlandı -> Kapak gerilimi MOSFET sürücüsünü aktif et (GPIO 1)
  pinMode(KAPAK_MOSFET_PIN, OUTPUT);
  digitalWrite(KAPAK_MOSFET_PIN, HIGH);
  delay(100); // Gerilimin oturması için kısa bekleme

  delay(500);
  mp3.begin();
  mp3.setVolume(volume);

  // Serial.println("lcd temizleniyor...");
  lc.clearDisplay(MATRIX_A);
  lc.clearDisplay(MATRIX_B);

  delay(500);
  // Serial.println("ESP32 Dual-Core FreeRTOS başlıyor...");

  // Core 0’a görev atama
  xTaskCreatePinnedToCore(TaskCore0Kodu, "TaskCore 0", 8192, NULL, 1,
                          &TaskCore0, 0);
  // Core 1’e görev atama
  xTaskCreatePinnedToCore(TaskCore1Kodu, "TaskCore 1", 8192, NULL, 1,
                          &TaskCore1, 1);
}

// ---------- Yardımcı Fonksiyonlar ----------
bool wifiMesgul =
    false; // WiFi işlemleri sırasında sensör okumayı durdurmak için flag
bool donanimIslemleriAktif =
    false; // Donanım işlemleri (Fan/Işık) sırasında TaskCore1'in LED'leri
           // kapatmasını engellemek için
volatile bool loadingAnimasyonAktif =
    false; // Veri gönderimi sırasında loading animasyonu için

void VeriGonderimi() {
  wifiMesgul = true;            // Sensör okumayı durdur
  donanimIslemleriAktif = true; // TaskCore1'in LED'lere müdahalesini engelle
  lc.clearDisplay(MATRIX_A);    // Üst ekran boş kalsın
  lc.clearDisplay(MATRIX_B);

  // 1. Önce Wi-Fi bağlantısını başlat
  WifiBaslat();

  bool basarili = false;
  // 2. İnternet bağlantısı aktif ise Supabase veri gönderme animasyonunu başlat
  if (wifiStatus && WiFi.status() == WL_CONNECTED) {
    Serial.println("[Veri] İnternet aktif. Supabase gönderim animasyonu başlatılıyor...");
    lc.clearDisplay(MATRIX_A);
    lc.clearDisplay(MATRIX_B);
    dotMatrisWaitGoster();        // A displayinde wait sembolü göster
    loadingAnimasyonAktif = true; // Sadece internet aktif olduğunda başlat!

    basarili = DatayiGonder(cihazId, fircaId);

    loadingAnimasyonAktif = false; // Gönderim bitince animasyonu durdur
  } else {
    Serial.println("[Veri] İnternet bağlantısı yok, veri gönderilemedi!");
  }

  WifiDurdur(); // Her durumda WiFi'yi kapat

  // 3. Dot matriste veri gönderim sonucunu göster (Başarılı: ✔, Başarısız: ✖)
  dotMatrisVeriGonderimSonuc(basarili);

  wifiMesgul = false;            // Sensör okumaya devam et
  donanimIslemleriAktif = false; // TaskCore1 normale dönebilir
}

void DonanimIslemleri() {
  donanimIslemleriAktif = true; // TaskCore1'in LED'lere müdahalesini engelle
  Serial.println("[DONANIM] Fan ve Mavi Işık açılıyor, A ve B displayinde pervane animasyonu başlıyor...");

  // 1. Fan ve Mavi Işık Aç
  digitalWrite(FAN_PIN, HIGH);
  for (int i = 0; i < LED_COUNT; i++) {
    strip.setPixelColor(i, 0, 0, 255); // Mavi
  }
  strip.show();

  int frame = 0;

  // 2. Fan Çalışırken (5 sn) A displayinde pervane hızlı dönsün
  unsigned long fanStart = millis();
  while (millis() - fanStart < 5000) {
    dotMatrisPervaneAdimi(frame++);
    delay(80); // 80ms hızında hızlı dönüş
  }

  digitalWrite(FAN_PIN, LOW);
  Serial.println("[DONANIM] Fan kapandı, mavi ışık sönene kadar pervane devam ediyor...");

  // 3. Fan kapandıktan sonra Mavi Işık sönene kadar (3 sn daha) pervane dönmeye devam etsin
  unsigned long lightStart = millis();
  while (millis() - lightStart < 3000) {
    dotMatrisPervaneAdimi(frame++);
    delay(100); // 100ms hızında dönüş
  }

  // 4. Mavi Işığı Söndür ve A displayini temizle
  for (int i = 0; i < LED_COUNT; i++) {
    strip.setPixelColor(i, 0, 0, 0); // Söndür
  }
  strip.show();

  lc.clearDisplay(MATRIX_A); // Pervaneleri söndür
  lc.clearDisplay(MATRIX_B);
  Serial.println("[DONANIM] Mavi ışık kapandı, pervane animasyonu sonlandı.");

  donanimIslemleriAktif = false; // TaskCore1 normale dönebilir
}

volatile bool muzikBaslatIstegi = false;
volatile bool muzikDurdurIstegi = false;

void BitirmeIslemleri() {
  // Kum saatini durdur ve ekranı temizle
  calis = false;
  lc.clearDisplay(MATRIX_A);
  lc.clearDisplay(MATRIX_B);

  // Müziği Core 0 üzerinden güvenle durdur ve bitmesini bekle
  muzikDurdurIstegi = true;
  while (muzikDurdurIstegi) {
    delay(10);
  }
  VeriGonderimi();
  // Serial.println("5 saniye bekleme...");
  delay(5000);
  DonanimIslemleri();
}

bool fircalamaAktif = false;
bool fircalamaYapildi = false; // Yeni flag: Fırçalama yapıldı mı?

// ---------- Görev Fonksiyonları ----------
// VS1053 kütüphanesinden gelen iç bilgi/hata mesajlarını seri porta yazdırır
void vs1053_info(const char *info) {
  Serial.print("[VS1053-INFO] ");
  Serial.println(info);
}

// Şarkı kendiliğinden tamamen bittiğinde VS1053 kütüphanesi bu fonksiyonu çağırır
void vs1053_eof_mp3(const char *info) {
  // Manuel durdurma yapılıyorsa veya şarkı henüz yeni başladıysa (10 sn'den az) döngüyü engelle!
  if (!sarkiDurduruluyor && fircalamaAktif && (millis() - sonSarkiBaslamaZamani > 10000)) {
    Serial.println("[MP3] Şarkı normal süresinde bitti. Sıradaki şarkı talep edildi.");
    sonrakiSarkiyaGec = true;
  }
}

void TaskCore0Kodu(void *pvParameters) {
  int animIndex = 0;
  unsigned long lastAnimUpdate = 0;

  while (true) {
    // Tüm SD Kart ve MP3 işlemleri Core 0 üzerinde çakışmasız yürütülür
    if (muzikDurdurIstegi) {
      MuzikDurdurFnc();
      muzikDurdurIstegi = false;
    }

    if (muzikBaslatIstegi) {
      muzikBaslatIstegi = false;
      if (klasor != "") {
        playRandomSongFromFolder(klasor.c_str());
      }
    }

    if (sonrakiSarkiyaGec) {
      sonrakiSarkiyaGec = false;
      if (fircalamaAktif && klasor != "") {
        playRandomSongFromFolder(klasor.c_str());
      }
    }

    if (mp3.isRunning()) {
      mp3.loop();
    }

    // Loading Animasyonu (Veri Gönderimi Sırasında hem LED hem Dot Matrix)
    if (loadingAnimasyonAktif) {
      if (millis() - lastAnimUpdate > 100) { // 100ms hızında
        lastAnimUpdate = millis();
        strip.clear();
        strip.setPixelColor(animIndex, 0, 255, 0); // Yeşil
        strip.show();
        dotMatrisVeriGonderimAnimasyonu(animIndex);
        animIndex = (animIndex + 1) % LED_COUNT;
      }
    }

    // MP3 çalma işlemi için çok kısa bir gecikme veya hiç gecikme olmamalıdır.
    // Ancak Watchdog Timer'ı tetiklememek için çok kısa bir delay ekliyoruz.
    vTaskDelay(1 / portTICK_PERIOD_MS);
  }
}

// ---------- Görev Fonksiyonları ----------
// Belirtilen pinden n defa okuma alıp ortalamasını döndürür (Gürültü ve geçiş filtresi)
int stabilAnalogOku(int pin, int ornekSayisi = 5, int beklemeMs = 20) {
  long toplam = 0;
  for (int i = 0; i < ornekSayisi; i++) {
    toplam += analogRead(pin);
    if (beklemeMs > 0) delay(beklemeMs);
  }
  return (int)(toplam / ornekSayisi);
}

void TaskCore1Kodu(void *pvParameters) {
  unsigned long lastPrintTime = 0;
  static int secilenFircaNo = 0;
  static int sonGosterilenFircaNo = 0;
  static bool herhangiBirFircaDisarida = false;
  static int fircalarYerindeSayaci = 0;
  static unsigned long lastNoteAnimTime = 0;
  static int noteAnimStep = 0;

  while (true) {
    // Kapak MOSFET sürücüsünün sürekli aktif (HIGH) kalmasını sağla
    digitalWrite(KAPAK_MOSFET_PIN, HIGH);

    // --- SENSÖR OKUMA VE BUTONLAR (TaskCore0'dan taşındı) ---
    b1 = digitalRead(BTNPLAY_PIN);
    b2 = digitalRead(BTNSTOP_PIN);
    b3 = digitalRead(BTNREFRESH_PIN);
    kapak = digitalRead(KAPAK_PIN);
    bool kapakAcik = (kapak == KAPAK_ACIK_SEVIYE);

    // WiFi meşgul değilse sensörleri oku
    static int lastKapak = -1;
    static unsigned long kapakAcilmaZamani = 0;
    static bool sensorDataReady = false;

    if (kapak != lastKapak) {
      Serial.print("[KAPAK] Durum Degisti: ");
      Serial.println(kapakAcik ? "ACIK (Sensor: 1)" : "KAPALI (Sensor: 0)");
      lastKapak = kapak;

      if (!kapakAcik) {
        fircalarYerindeSayaci = 0; // Kapak kapandığında sahte yerinde sayımını anında sıfırla
      }

      // Kapak ilk açıldığında beyaz LED'leri yak ve fırça değerlerini 5 defa kontrol et
      if (kapakAcik) {
        // 1. Önce beyaz LED'leri hemen yak
        for (int i = 0; i < LED_COUNT; i++) {
          strip.setPixelColor(i, 255, 255, 255);
        }
        strip.show();

        Serial.println("[KAPAK] Beyaz LED'ler açıldı. Işığın oturması bekleniyor (150ms)...");
        delay(150);

        // 2. Fırçaların değerlerini 5 defa kontrol et / örnekle
        Serial.println("[KAPAK] Fırça sensörleri 5 defa kontrol ediliyor...");
        firca1Ldr = stabilAnalogOku(FIRCA1_PIN, 5, 20);
        firca2Ldr = stabilAnalogOku(FIRCA2_PIN, 5, 20);
        firca3Ldr = stabilAnalogOku(FIRCA3_PIN, 5, 20);
        firca4Ldr = stabilAnalogOku(FIRCA4_PIN, 5, 20);
        firca5Ldr = stabilAnalogOku(FIRCA5_PIN, 5, 20);
        ortamLdr  = stabilAnalogOku(ORTAMLDR_PIN, 5, 20);
        LdrEsikDeger = ortamLdr + 500;
        sensorDataReady = true;

        Serial.print("[KAPAK] 5 Kontrol Tamamlandı -> Ortam: ");
        Serial.print(ortamLdr);
        Serial.print(" | Fircalar (1-5): ");
        Serial.print(firca1Ldr); Serial.print(" ");
        Serial.print(firca2Ldr); Serial.print(" ");
        Serial.print(firca3Ldr); Serial.print(" ");
        Serial.print(firca4Ldr); Serial.print(" ");
        Serial.println(firca5Ldr);
      }
    }

    // Sensörleri WiFi meşgul değilken sürekli oku
    if (!wifiMesgul && !donanimIslemleriAktif) {
      firca1Ldr = analogRead(FIRCA1_PIN);
      firca2Ldr = analogRead(FIRCA2_PIN);
      firca3Ldr = analogRead(FIRCA3_PIN);
      firca4Ldr = analogRead(FIRCA4_PIN);
      firca5Ldr = analogRead(FIRCA5_PIN);
      ortamLdr = analogRead(ORTAMLDR_PIN);
      LdrEsikDeger = ortamLdr + 500;
      sensorDataReady = true;
    }

    // Kapak Açıkken Fırça Kontrolü
    if (kapakAcik && sensorDataReady) {
      int thrFirca = min(3800, (int)ortamLdr + 1500);
      int thrFirca5 = min(3500, (int)ortamLdr + 1000);

      herhangiBirFircaDisarida = false;
      secilenFircaNo = 0;

      if (firca1Ldr <= thrFirca) {
        // Serial.println("Fırça 1 Dışarıda");
        fircaId = "Firca1";
        klasor = "/1";
        secilenFircaNo = 1;
        herhangiBirFircaDisarida = true;
      } else if (firca2Ldr <= thrFirca) {
        // Serial.println("Fırça 2 Dışarıda");
        fircaId = "Firca2";
        klasor = "/2";
        secilenFircaNo = 2;
        herhangiBirFircaDisarida = true;
      } else if (firca3Ldr <= thrFirca) {
        // Serial.println("Fırça 3 Dışarıda");
        fircaId = "Firca3";
        klasor = "/3";
        secilenFircaNo = 3;
        herhangiBirFircaDisarida = true;
      } else if (firca4Ldr <= thrFirca) {
        // Serial.println("Fırça 4 Dışarıda");
        fircaId = "Firca4";
        klasor = "/4";
        secilenFircaNo = 4;
        herhangiBirFircaDisarida = true;
      } else if (firca5Ldr <= thrFirca5) {
        // Serial.println("Fırça 5 Dışarıda");
        fircaId = "Firca5";
        klasor = "/5";
        secilenFircaNo = 5;
        herhangiBirFircaDisarida = true;
      }

      if (herhangiBirFircaDisarida) {
        fircalarYerindeSayaci = 0; // Fırça dışarıda olduğu sürece sayacı sıfırla

        if (!fircalamaAktif) {
          Serial.print("[FIRCA] Fırça çıkarıldı (");
          Serial.print(fircaId);
          Serial.println("), müzik başlatılıyor...");
          fircalamaAktif = true;
          muzikBaslatIstegi = true; // Core 0'a güvenli müzik başlatma komutu
        }

        // Kum saati henüz başlamadıysa A displayinde fırça numarasını göster
        if (!calis && secilenFircaNo != sonGosterilenFircaNo) {
          sonGosterilenFircaNo = secilenFircaNo;
          Serial.print("[DISPLAY-A] Fırça Numarası: ");
          Serial.print(secilenFircaNo);
          Serial.println(" | [DISPLAY-B] Müzik Notası Sallanıyor");
          dotMatrisRakamGoster(secilenFircaNo);
        }
      } else {
        // Tüm fırçalar yerinde görünüyor, ancak:
        // 1. Kapak tamamen açık olmalı!
        // 2. Ortam LDR'si aydınlık olmalı (kapak kapanırken ortam kararır, bu sahte algılamayı önler!)
        if (kapakAcik && ortamLdr < 2000) {
          fircalarYerindeSayaci++;
        } else {
          fircalarYerindeSayaci = 0;
        }

        if (fircalarYerindeSayaci >= 15) {
          fircalarYerindeSayaci = 0;

          // Çıkarılmış olan fırçanın LDR sensörünü 5 kez stabil ölçerek gerçekten yerine oturduğunu teyit et
          bool gercektenYerinde = true;

          // Ortam ışığı hala aydınlık mı ve kapak hala açık mı? (Kapak kapanıyorsa ortam kararır!)
          int guncelOrtam = stabilAnalogOku(ORTAMLDR_PIN, 5, 20);
          if (guncelOrtam > 2000 || !kapakAcik) {
            gercektenYerinde = false; // Kapak kapanıyor veya ortam karanlık, sahte algılama!
            Serial.println("[FIRCA] Yanlış alarm: Ortam karanlık veya kapak kapanıyor, fırça yerine kondu sayılamaz!");
          }

          if (sonGosterilenFircaNo > 0 && gercektenYerinde) {
            int kontrolPin = (sonGosterilenFircaNo == 1) ? FIRCA1_PIN :
                             (sonGosterilenFircaNo == 2) ? FIRCA2_PIN :
                             (sonGosterilenFircaNo == 3) ? FIRCA3_PIN :
                             (sonGosterilenFircaNo == 4) ? FIRCA4_PIN : FIRCA5_PIN;
            int dogrulamaDegeri = stabilAnalogOku(kontrolPin, 5, 20);
            int esik = min(3800, (sonGosterilenFircaNo == 5) ? (guncelOrtam + 1000) : (guncelOrtam + 1500));
            if (dogrulamaDegeri <= esik) {
              gercektenYerinde = false; // Fırça hala dışarıda, yanlış alarm!
              Serial.println("[FIRCA] Yanlış alarm: Fırça hala dışarıda, veri gönderimi yapılmıyor.");
            }
          }

          // YALNIZCA FIRÇA KESİN OLARAK YERİNE KONDUYSA BİTİR VE VERİ GÖNDER!
          if (gercektenYerinde && (mp3.isRunning() || isPlaying || fircalamaAktif)) {
            Serial.println("[FIRCA] Fırça kesin olarak yerine kondu -> Müzik durduruluyor ve veri gönderiliyor...");
            sonGosterilenFircaNo = 0;
            muzikDurdurIstegi = true; // Core 0'a güvenli müzik durdurma komutu
            while (muzikDurdurIstegi) {
              delay(10);
            }

            // Kum saatini durdur ve dot matrix ekranları söndür
            if (calis) {
              calis = false;
              lc.clearDisplay(MATRIX_A);
              lc.clearDisplay(MATRIX_B);
            } else {
              dotMatrisRakamGoster(0); // A displayindeki rakamı temizle
              lc.clearDisplay(MATRIX_B); // B displayindeki notayı temizle
            }

            if (fircalamaAktif) {
              VeriGonderimi();
              fircalamaAktif = false;
              fircalamaYapildi = true; // Kapak kapanınca fan çalışsın diye işaretle
            }

            fircaId = "";
            klasor = "";
          }
        }
      }
    }
    // Kapak kapandıysa durum kontrolü
    else if (!kapakAcik) {
      fircalarYerindeSayaci = 0; // Kapak kapalıyken sayaç sıfır tutulur

      if (fircalamaYapildi && !fircalamaAktif) {
        Serial.println("[KAPAK] Kapak kapandı ve fırçalama tamamlanmıştı -> Donanım kurutma işlemleri (Fan & Mavi Işık) başlatılıyor.");
        DonanimIslemleri();
        fircalamaYapildi = false;
      } else if (fircalamaAktif) {
        // Çıkarılan fırça yerine gelmediyse kapak kapansa bile veri gönderimi ve donanım işlemlerine GEÇME!
        static unsigned long lastUyariZamani = 0;
        if (millis() - lastUyariZamani > 3000) {
          lastUyariZamani = millis();
          Serial.println("[KAPAK] Fırça henüz yerine konmadı! Kapak kapansa bile veri gönderimi veya kurutmaya geçilmiyor.");
        }
      }
    }

    // --- BUTON İŞLEMLERİ (Birleştirilmiş) ---

    // 5 Saniye Basılı Tutma ile AP Moduna Geçiş (PLAY + STOP aynı anda)
    static unsigned long apBasiliTutmaZamani = 0;
    static int sonSaniyeSayaci = -1;

    if (b1 == HIGH && b2 == HIGH) {
      if (apBasiliTutmaZamani == 0) {
        apBasiliTutmaZamani = millis();
        sonSaniyeSayaci = 0;
        Serial.println("\n[AP-KURULUM] Play ve Stop butonlarina basildi! AP moduna gecmek icin 5 sn basili tutun...");
      }

      unsigned long basiliSure = millis() - apBasiliTutmaZamani;
      int gecenSaniye = basiliSure / 1000;

      // Her geçen saniye için seri porta ve LED'lere geri bildirim
      if (gecenSaniye != sonSaniyeSayaci && gecenSaniye <= 5) {
        sonSaniyeSayaci = gecenSaniye;
        Serial.print("[AP-KURULUM] Basili tutuluyor: ");
        Serial.print(gecenSaniye);
        Serial.println("/5 saniye...");

        // NeoPixel LED'leri adım adım yakarak görsel sayaç göster
        int yakilacakLed = map(basiliSure, 0, 5000, 0, LED_COUNT);
        strip.clear();
        for (int i = 0; i < yakilacakLed && i < LED_COUNT; i++) {
          strip.setPixelColor(i, 255, 160, 0); // Sarı/Turuncu ilerleme çubuğu
        }
        strip.show();
      }

      if (basiliSure >= 5000) {
        Serial.println("\n==================================================");
        Serial.println("[AP-KURULUM] 5 SANİYE DOLDU! AP MODUNA GEÇİLİYOR...");
        Serial.println("==================================================");

        // LED'leri tamamen sarı yap
        for (int i = 0; i < LED_COUNT; i++) {
          strip.setPixelColor(i, 255, 160, 0);
        }
        strip.show();

        // Kum saatini ve müziği durdur
        calis = false;
        lc.clearDisplay(MATRIX_A);
        lc.clearDisplay(MATRIX_B);
        muzikDurdurIstegi = true;
        while (muzikDurdurIstegi) {
          delay(10);
        }

        wifiMesgul = true;
        donanimIslemleriAktif = true;

        // AP Modunu başlat ve bekleme döngüsüne gir (Kayıt olunca yeniden başlar)
        apModunuBaslat();
        apModuDongusu();
      }
    } else {
      // Butonlar bırakıldıysa sayacı sıfırla
      apBasiliTutmaZamani = 0;
      sonSaniyeSayaci = -1;

      // OYNAT BUTONU (Yalnızca tek basıldığında) - Kum saatini başlatır
      if (b1 == HIGH) {
        if (millis() - lastDebounce > DEBOUNCE_THRESHOLD) {
          lastDebounce = millis();
          Serial.println("[BUTON] Play -> 2 dk (120 sn) Kum Saati baslatiliyor");
          setHourglassDuration(120); // 2 dakika kum saati
          for (byte i = 0; i < 2; i++) {
            lc.shutdown(i, false);
            lc.setIntensity(i, 2);
          }
          resetTime();
          calis = true; // Kum saatini başlat

          // Eğer fırça dışarıdaysa ve müzik durdurulmuşsa müziği de yeniden başlat
          if (fircalamaAktif && !mp3.isRunning() && !isPlaying) {
            Serial.println("[BUTON] Play -> Fırça dışarıda ve müzik durmuştu, müzik yeniden başlatılıyor...");
            muzikBaslatIstegi = true;
          }
        }
      }

      // DURDUR BUTONU (Yalnızca tek basıldığında)
      // 1) Kum saati başlamamışsa -> Müziği durdurur
      // 2) Kum saati çalışıyorsa:
      //    - İlk basışta -> Kum saatini durdurur (müzik devam eder)
      //    - İkinci basışta -> Müziği durdurur
      if (b2 == HIGH) {
        if (millis() - lastDebounce > DEBOUNCE_THRESHOLD) {
          lastDebounce = millis();

          int aktifFirca = (secilenFircaNo > 0) ? secilenFircaNo : sonGosterilenFircaNo;
          bool fircaDisarida = (fircalamaAktif && aktifFirca > 0);

          if (calis) {
            // DURUM 1: Kum saati çalışıyorken basıldı -> İLK BASIŞ: Yalnızca kum saatini durdur
            Serial.println("[BUTON] Stop (1. Basış) -> Kum Saati Durduruldu, Müzik Devam Ediyor");
            calis = false;

            if (fircaDisarida) {
              Serial.print("[BUTON] Fırça hala dışarıda (Fırça ");
              Serial.print(aktifFirca);
              Serial.println(") -> A displayinde rakam, B displayinde nota animasyonu gösterilmeye devam ediyor.");

              lc.clearDisplay(MATRIX_A);
              lc.clearDisplay(MATRIX_B);
              dotMatrisRakamGoster(aktifFirca);
              dotMatrisNotaSalla(0);
              sonGosterilenFircaNo = aktifFirca;
              noteAnimStep = 1;
              lastNoteAnimTime = millis();
            } else {
              lc.clearDisplay(MATRIX_A);
              lc.clearDisplay(MATRIX_B);
              sonGosterilenFircaNo = 0;
            }
          } else {
            // DURUM 2: Kum saati çalışmıyorken basıldı (Play'e hiç basılmamış veya 2. kez Stop'a basılmış) -> MÜZİĞİ DURDUR
            if (fircaDisarida) {
              Serial.println("[BUTON] Stop (Kum saati kapalıyken / 2. basış) -> Müzik Durduruluyor");
              muzikDurdurIstegi = true;  // Core 0'a müzik durdurma komutu
              lc.clearDisplay(MATRIX_B); // Müzik durduğu için nota animasyonunu söndür
              dotMatrisRakamGoster(aktifFirca); // A displayindeki fırça numarası kalsın
            } else {
              // Fırça yerindeyse ve kum saati zaten kapalıysa ekranları temizle
              lc.clearDisplay(MATRIX_A);
              lc.clearDisplay(MATRIX_B);
              sonGosterilenFircaNo = 0;
            }
          }
        }
      }
    }

    // YENILE / REFRESH BUTONU
    static unsigned long lastRefreshDebounce = 0;
    if (b3 == HIGH) {
      if (millis() - lastRefreshDebounce > DEBOUNCE_THRESHOLD) {
        lastRefreshDebounce = millis();
        int aktifFirca = (secilenFircaNo > 0) ? secilenFircaNo : sonGosterilenFircaNo;
        bool fircaDisarida = (fircalamaAktif && (herhangiBirFircaDisarida || aktifFirca > 0));

        if (fircaDisarida) {
          // Fırça yerinde değilken: Şarkıyı değiştir!
          Serial.println("[BUTON] Fırça dışarıda + Refresh -> Sıradaki şarkıya geçiliyor...");
          sonrakiSarkiyaGec = true;
        } else if (!kapakAcik) {
          // Kapak KAPALI ise: 20 sn Kum Saati
          Serial.println("[BUTON] Kapak Kapali + Refresh -> 20 sn Kum Saati baslatiliyor");
          setHourglassDuration(20); // 20 saniye kum saati
          for (byte i = 0; i < 2; i++) {
            lc.shutdown(i, false);
            lc.setIntensity(i, 2);
          }
          resetTime();
          calis = true; // Kum saatini başlat
        } else {
          // Kapak AÇIK ve fırçalar yerinde ise: Resetle
          Serial.println("[BUTON] Kapak Acik + Refresh -> Kum Saati Sifirlandi");
          for (byte i = 0; i < 2; i++) {
            lc.shutdown(i, false);
            lc.setIntensity(i, 2);
          }
          calis = false;
          resetTime();
        }
      }
    }

    // --- LED VE KUM SAATİ ANİMASYONLARI ---

    // Normal Kapak Durumu (Donanım işlemleri aktif değilse ve loading
    // animasyonu aktif değilse)
    if (!donanimIslemleriAktif && !loadingAnimasyonAktif) {
      if (kapakAcik) { // Kapak Açık -> Beyaz LED'ler yanar
        for (int i = 0; i < LED_COUNT; i++) {
          strip.setPixelColor(i, 255, 255, 255);
        }
      } else { // Kapak Kapalı -> LED'ler söner
        for (int i = 0; i < LED_COUNT; i++) {
          strip.setPixelColor(i, 0, 0, 0);
        }
      }
      strip.show();
    }

    // Periyodik Sensör Bilgisi Yazdırma (1 saniyede bir)
    if (!wifiMesgul && (millis() - lastPrintTime > 1000)) {
      bilgileriYazdir();
      lastPrintTime = millis();
    }
    /* Kapak durum sonu*/

    /*Kum saati baslangıcı*/
    if (calis) {
      delay(DELAY_FRAME); // Bu delay animasyon hızı için gerekli
      gravity = getGravity();
      lc.setRotation((ROTATION_OFFSET + gravity) % 360);
      // //Serial.println(gravity);
      bool moved = updateMatrix();
      bool dropped = dropParticle();
    } else if (fircalamaAktif && !wifiMesgul && !donanimIslemleriAktif && (isPlaying || mp3.isRunning())) {
      // Kum saati çalışmıyorken, fırça henüz yerine konmamışsa VE müzik çalıyorsa
      // B displayinde müzik notası sallanmaya kesintisiz devam etsin
      if (millis() - lastNoteAnimTime > 250) { // 250ms ritmik sallanma hızı
        lastNoteAnimTime = millis();
        dotMatrisNotaSalla(noteAnimStep++);
      }
    }
    /*Kum saati bitis*/

    vTaskDelay(50 / portTICK_PERIOD_MS);
  }
}

// ---------- Loop ----------
void loop() {

  // Genellikle boş bırakılır, çünkü FreeRTOS görevleri kullanılıyor
}

void bilgileriYazdir() {
  int rawKapak = digitalRead(KAPAK_PIN);
  bool acik = (rawKapak == KAPAK_ACIK_SEVIYE);
  Serial.print("[SENSOR] KapakRaw(Pin21): ");
  Serial.print(rawKapak);
  Serial.print(acik ? " [ACIK]" : " [KAPALI]");
  Serial.print(" | Butonlar (b1,b2,b3): ");
  Serial.print(b1);
  Serial.print(",");
  Serial.print(b2);
  Serial.print(",");
  Serial.print(b3);
  Serial.print(" | OrtamLDR: ");
  Serial.print(ortamLdr);
  Serial.print(" | Fircalar (1-5): ");
  Serial.print(firca1Ldr);
  Serial.print(" ");
  Serial.print(firca2Ldr);
  Serial.print(" ");
  Serial.print(firca3Ldr);
  Serial.print(" ");
  Serial.print(firca4Ldr);
  Serial.print(" ");
  Serial.println(firca5Ldr);
}