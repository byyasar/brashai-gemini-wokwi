#ifndef CONFIG_H
#define CONFIG_H

#include <Arduino.h>
#include <Preferences.h>

// Global Değişken Bildirimleri
extern String wifi_ssid;
extern String wifi_password;
extern String supabase_url;
extern String supabase_key;
extern String cihazId;

// Örnek Varsayılan Değerler (Kendi bilgilerinizi src/config.h dosyasına girin)
#define DEFAULT_SSID "Wokwi-GUEST"
#define DEFAULT_PASS ""
#define DEFAULT_SUPABASE_URL "https://YOUR_PROJECT.supabase.co"
#define DEFAULT_SUPABASE_KEY "YOUR_SUPABASE_ANON_KEY"
#define DEFAULT_CIHAZ_ID "Cihaz1"

// NVS Hafızasından (Preferences) Ayarları Yükle
inline void ayarlariYukle() {
  Preferences prefs;
  prefs.begin("brashai", true);
  wifi_ssid = prefs.getString("ssid", DEFAULT_SSID);
  wifi_password = prefs.getString("pass", DEFAULT_PASS);
  supabase_url = prefs.getString("sb_url", DEFAULT_SUPABASE_URL);
  if (supabase_url.isEmpty()) {
    supabase_url = DEFAULT_SUPABASE_URL;
  }
  supabase_key = prefs.getString("sb_key", DEFAULT_SUPABASE_KEY);
  if (supabase_key.isEmpty()) {
    supabase_key = DEFAULT_SUPABASE_KEY;
  }
  cihazId = prefs.getString("cihaz_id", DEFAULT_CIHAZ_ID);
  prefs.end();
}

// NVS Hafızasına (Preferences) Yeni Ayarları Kaydet
inline void ayarlariKaydet(const String &new_ssid, const String &new_pass,
                           const String &new_url, const String &new_key,
                           const String &new_cihaz_id) {
  Preferences prefs;
  prefs.begin("brashai", false);
  prefs.putString("ssid", new_ssid);
  prefs.putString("pass", new_pass);
  prefs.putString("sb_url", new_url);
  prefs.putString("sb_key", new_key);
  prefs.putString("cihaz_id", new_cihaz_id);
  prefs.end();
}

#endif // CONFIG_H
