#ifndef WIFI_ISLEMLERI_H
#define WIFI_ISLEMLERI_H

#include "Arduino.h"
#include "config.h"
#include <DNSServer.h>
#include <WebServer.h>
#include <WiFi.h>

// Global değişken tanımları
String wifi_ssid = "";
String wifi_password = "";
String supabase_url = "";
String supabase_key = "";
String cihazId = "Cihaz1";

bool wifiStatus = false;

// AP ve Web Sunucu Tanımları
const char *AP_SSID = "BrashAI";
const char *AP_PASS = "12345678";

const byte DNS_PORT = 53;
IPAddress apIP(192, 168, 4, 1);
IPAddress netMsk(255, 255, 255, 0);

DNSServer dnsServer;
WebServer server(80);

bool apModuAktif = false;
bool ayarKaydedildi = false;
String tarananAglarHTML = "";

// Çevredeki Wi-Fi ağlarını tara ve HTML seçeneklerine dönüştür
void aglariTara() {
  Serial.println("Çevredeki Wi-Fi ağları taranıyor...");
  int n = WiFi.scanNetworks();
  tarananAglarHTML = "";

  if (n <= 0) {
    tarananAglarHTML = "<option value=''>Ağ bulunamadı</option>";
  } else {
    for (int i = 0; i < n; ++i) {
      String agAdi = WiFi.SSID(i);
      int rssi = WiFi.RSSI(i);
      tarananAglarHTML += "<option value='" + agAdi + "'>" + agAdi + " (" +
                          String(rssi) + " dBm)</option>";
    }
  }
  Serial.print(n);
  Serial.println(" adet ağ bulundu.");
}

// HTML Kurulum Sayfası
String htmlSayfasiOlustur() {
  String html = R"rawhtml(
<!DOCTYPE html>
<html lang="tr">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>BrashAI Kurulum</title>
  <style>
    * { box-sizing: border-box; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; }
    body { background: #0f172a; color: #f8fafc; margin: 0; padding: 20px; display: flex; justify-content: center; }
    .card { background: #1e293b; border-radius: 16px; padding: 24px; width: 100%; max-width: 440px; box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.4); }
    h2 { margin-top: 0; color: #38bdf8; text-align: center; }
    p.desc { font-size: 14px; color: #94a3b8; text-align: center; margin-bottom: 24px; }
    .form-group { margin-bottom: 16px; }
    label { display: block; font-size: 13px; font-weight: 600; margin-bottom: 6px; color: #cbd5e1; }
    input, select { width: 100%; padding: 12px; border-radius: 8px; border: 1px solid #334155; background: #0f172a; color: #fff; font-size: 14px; outline: none; }
    input:focus, select:focus { border-color: #38bdf8; ring: 2px solid #38bdf8; }
    .hint { font-size: 12px; color: #64748b; margin-top: 4px; }
    button { width: 100%; padding: 14px; background: #0284c7; color: white; border: none; border-radius: 8px; font-size: 16px; font-weight: bold; cursor: pointer; transition: background 0.2s; margin-top: 10px; }
    button:hover { background: #0369a1; }
    .refresh-link { font-size: 12px; color: #38bdf8; text-decoration: none; float: right; }
  </style>
  <script>
    function agSecildi(select) {
      if (select.value !== "") {
        document.getElementById("ssidInput").value = select.value;
      }
    }
  </script>
</head>
<body>
  <div class="card">
    <h2>BrashAI Kurulum</h2>
    <p class="desc">Cihazınızı Wi-Fi ve Supabase veritabanına bağlamak için bilgileri giriniz.</p>
    
    <form action="/save" method="POST">
      <div class="form-group">
        <label>Çevredeki Wi-Fi Ağları: <a class="refresh-link" href="/rescan">Yenile</a></label>
        <select onchange="agSecildi(this)">
          <option value="">-- Listeden Ağ Seçin --</option>
  )rawhtml";

  html += tarananAglarHTML;

  html += R"rawhtml(
        </select>
      </div>

      <div class="form-group">
        <label for="ssidInput">Wi-Fi Adı (SSID):</label>
        <input type="text" id="ssidInput" name="ssid" value=")rawhtml";
  html += wifi_ssid;
  html += R"rawhtml(" required placeholder="Wi-Fi adını yazın">
      </div>

      <div class="form-group">
        <label>Wi-Fi Şifresi:</label>
        <input type="password" name="password" value=")rawhtml";
  html += wifi_password;
  html += R"rawhtml(" placeholder="Wi-Fi şifresi">
      </div>

      <div class="form-group">
        <label>Supabase URL:</label>
        <input type="text" name="supabase_url" value=")rawhtml";
  html += supabase_url;
  html += R"rawhtml(" required placeholder="https://xyz.supabase.co">
        <div class="hint">Örnek: https://xyz.supabase.co</div>
      </div>

      <div class="form-group">
        <label>Supabase Key (Anon Key):</label>
        <input type="password" name="supabase_key" value=")rawhtml";
  html += supabase_key;
  html += R"rawhtml(" required placeholder="eyJhbGciOiJIUz...">
        <div class="hint">Supabase Anon / Public API Anahtarınız</div>
      </div>

      <div class="form-group">
        <label>Cihaz ID (Cihaz Adı):</label>
        <input type="text" name="cihaz_id" value=")rawhtml";
  html += cihazId;
  html += R"rawhtml(" required placeholder="Örn: Cihaz1">
        <div class="hint">Supabase tablosuna kaydedilecek cihaz kimliği (Varsayılan: Cihaz1)</div>
      </div>

      <button type="submit">Kaydet ve Yeniden Başlat</button>
    </form>
  </div>
</body>
</html>
  )rawhtml";

  return html;
}

// Kayıt İşlemi ve Yeniden Başlatma Yanıtı
void handleSave() {
  String new_ssid = server.arg("ssid");
  String new_pass = server.arg("password");
  String new_url = server.arg("supabase_url");
  String new_key = server.arg("supabase_key");
  String new_cihaz_id = server.arg("cihaz_id");

  new_ssid.trim();
  new_pass.trim();
  new_url.trim();
  new_key.trim();
  new_cihaz_id.trim();

  if (new_cihaz_id.isEmpty()) {
    new_cihaz_id = "Cihaz1";
  }

  // URL sonunda slash varsa temizle
  if (new_url.endsWith("/")) {
    new_url.remove(new_url.length() - 1);
  }

  ayarlariKaydet(new_ssid, new_pass, new_url, new_key, new_cihaz_id);

  String basariHtml = R"rawhtml(
<!DOCTYPE html>
<html lang="tr">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Ayarlar Kaydedildi</title>
  <style>
    body { background: #0f172a; color: #f8fafc; font-family: sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; text-align: center; }
    .box { background: #1e293b; padding: 30px; border-radius: 16px; max-width: 380px; }
    h2 { color: #4ade80; }
    p { color: #94a3b8; }
  </style>
</head>
<body>
  <div class="box">
    <h2>Ayarlar Kaydedildi!</h2>
    <p>BrashAI cihazı yeniden başlatılıyor ve Wi-Fi ağına bağlanıyor...</p>
    <p>Lütfen bekleyiniz.</p>
  </div>
</body>
</html>
  )rawhtml";

  server.send(200, "text/html", basariHtml);
  ayarKaydedildi = true;
}

// Captive Portal ve Root Yönlendirmeleri
void handleRoot() {
  server.send(200, "text/html", htmlSayfasiOlustur());
}

void handleRescan() {
  aglariTara();
  server.sendHeader("Location", "/", true);
  server.send(302, "text/plain", "");
}

void handleNotFound() {
  server.sendHeader("Location", String("http://") + apIP.toString(), true);
  server.send(302, "text/plain", "");
}

// AP Modunu Başlatma
void apModunuBaslat() {
  Serial.println("\n-------------------------------------------");
  Serial.println("Wi-Fi bağlantısı kurulamadı. AP Modu başlatılıyor...");
  Serial.println("-------------------------------------------");

  WiFi.disconnect(true);
  WiFi.mode(WIFI_AP_STA); // Hem AP yayını yap hem de ağ tarayabil
  WiFi.setTxPower(WIFI_POWER_11dBm); // Aşırı akım çekmesini engelle
  WiFi.softAPConfig(apIP, apIP, netMsk);
  WiFi.softAP(AP_SSID, AP_PASS);

  apModuAktif = true;

  // Çevredeki ağları başlangıçta bir kez tara
  aglariTara();

  // DNS Server Başlat (Tüm istekleri ESP32 IP'sine yönlendir - Captive Portal)
  dnsServer.setErrorReplyCode(DNSReplyCode::NoError);
  dnsServer.start(DNS_PORT, "*", apIP);

  // WebServer Rotaları
  server.on("/", HTTP_GET, handleRoot);
  server.on("/rescan", HTTP_GET, handleRescan);
  server.on("/save", HTTP_POST, handleSave);

  // Captive portal tespit rotaları (Android / iOS / Windows)
  server.on("/generate_204", HTTP_GET, handleRoot);
  server.on("/fwlink", HTTP_GET, handleRoot);
  server.on("/hotspot-detect.html", HTTP_GET, handleRoot);
  server.onNotFound(handleNotFound);

  server.begin();

  Serial.println("AP Modu Hazır!");
  Serial.print("Wi-Fi Adı (SSID) : ");
  Serial.println(AP_SSID);
  Serial.print("Şifre            : ");
  Serial.println(AP_PASS);
  Serial.print("Ayar Portalı IP  : http://");
  Serial.println(WiFi.softAPIP());
  Serial.println("Lütfen bu ağa bağlanıp ayarları giriniz...");
}

// AP Modu bekleme döngüsü
void apModuDongusu() {
  while (true) {
    dnsServer.processNextRequest();
    server.handleClient();

    if (ayarKaydedildi) {
      delay(2000);
      Serial.println("Cihaz yeniden başlatılıyor...");
      ESP.restart();
    }
    delay(10);
  }
}

/* Wi-Fi Bağlanma Fonksiyonu - 3 Kez Dener */
bool WifiBaglan(int maxRetries = 3, int timeout = 7000) {
  if (wifi_ssid.isEmpty()) {
    Serial.println("[Wi-Fi] Kayıtlı Wi-Fi SSID bulunamadı!");
    wifiStatus = false;
    dotMatrisWifiBasarisiz();
    return false;
  }

  if (WiFi.status() == WL_CONNECTED) {
    wifiStatus = true;
    dotMatrisWifiBasarili();
    return true;
  }

  WiFi.mode(WIFI_STA);
  delay(50);

  int animFrame = 0;
  for (int attempt = 1; attempt <= maxRetries; attempt++) {
    Serial.print("[Wi-Fi] '");
    Serial.print(wifi_ssid);
    Serial.print("' ağına bağlanılıyor... Deneme ");
    Serial.print(attempt);
    Serial.print("/");
    Serial.println(maxRetries);

    WiFi.begin(wifi_ssid.c_str(), wifi_password.c_str());
    unsigned long startAttemptTime = millis();

    while (WiFi.status() != WL_CONNECTED &&
           millis() - startAttemptTime < timeout) {
      dotMatrisWifiAnimasyonu(animFrame++);
      delay(300);
      Serial.print(".");
    }
    Serial.println();

    if (WiFi.status() == WL_CONNECTED) {
      Serial.println("[Wi-Fi] Başarıyla bağlanıldı!");
      Serial.print("[Wi-Fi] Cihaz IP adresi: ");
      Serial.println(WiFi.localIP());
      wifiStatus = true;
      dotMatrisWifiBasarili(); // Ekranda Wi-Fi + TİK ✔ göster
      return true;
    } else {
      Serial.print("[Wi-Fi] ");
      Serial.print(attempt);
      Serial.println(". deneme başarısız.");
      WiFi.disconnect(true);
      delay(500);
    }
  }

  Serial.println("[Wi-Fi] 3 deneme tamamlandı, bağlantı kurulamadı!");
  wifiStatus = false;
  dotMatrisWifiBasarisiz(); // Ekranda Çarpı ✖ göster
  return false;
}

// Veri gönderimi sırasında çağrılan standart WifiBaslat
// Bağlanamazsa otomatik olarak AP moduna geçer
void WifiBaslat() {
  if (WiFi.status() == WL_CONNECTED) {
    wifiStatus = true;
    return;
  }

  bool baglandi = WifiBaglan(3, 7000);
  if (!baglandi) {
    Serial.println("[Wi-Fi] Bağlantı sağlanamadı -> AP Moduna geçiliyor...");
    apModunuBaslat();
    apModuDongusu();
  }
}

void WifiDurdur() {
  WiFi.disconnect(true);
  WiFi.mode(WIFI_OFF);
  wifiStatus = false;
  Serial.println("[Wi-Fi] Bağlantı kapatıldı (Güç Tasarrufu).");
}

// İlk açılışta kontrol ve AP moduna geçiş
void WifiIlkAcilisKontrol() {
  ayarlariYukle();

  bool baglandi = false;
  if (!wifi_ssid.isEmpty()) {
    Serial.println("\n[Sistem] İlk açılış: Wi-Fi bağlantısı kontrol ediliyor (3 deneme)...");
    baglandi = WifiBaglan(3, 7000);
  } else {
    Serial.println("\n[Sistem] Kayıtlı Wi-Fi bilgisi bulunmuyor.");
  }

  if (!baglandi) {
    Serial.println("[Sistem] Wi-Fi ağına bağlanılamadı! AP Modu açılıyor...");
    // Wi-Fi'ye bağlanamadı -> AP Moduna geç
    apModunuBaslat();
    // Kullanıcı bilgileri girip yeniden başlatana kadar bu döngüde kalır
    apModuDongusu();
  } else {
    Serial.println("[Sistem] Wi-Fi bağlantı kontrolü başarılı!");
    // Veri gönderimine kadar pil tasarrufu için Wi-Fi kapatılıyor
    WifiDurdur();
  }
}

#endif // WIFI_ISLEMLERI_H
