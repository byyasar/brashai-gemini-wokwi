#ifndef SUPABASE_ISLEMLERI_H
#define SUPABASE_ISLEMLERI_H

#include "Arduino.h"
#include "config.h"
#include <HTTPClient.h>
#include <NTPClient.h>
#include <WiFi.h>
#include <WiFiClientSecure.h>
#include <WiFiUdp.h>

String tarih = "";
String saat = "";

// NTP Client Ayarları
WiFiUDP ntpUDP;
NTPClient timeClient(ntpUDP, "tr.pool.ntp.org",
                     3 * 3600); // Türkiye saati UTC+3

/*Tarih saat işlemleri baslangıcı*/
void printLocalTime() {
  timeClient.begin();
  int retry = 0;
  while (!timeClient.update() && retry < 3) {
    timeClient.forceUpdate();
    retry++;
    delay(300);
  }

  time_t epochTime = timeClient.getEpochTime();
  struct tm tmStruct;
  if (gmtime_r(&epochTime, &tmStruct) != nullptr) {
    int currentYear = tmStruct.tm_year + 1900;
    int currentMonth = tmStruct.tm_mon + 1;
    int monthDay = tmStruct.tm_mday;

    // Tarih formatı: D/M/YYYY
    tarih = String(monthDay) + "/" + String(currentMonth) + "/" +
            String(currentYear);
    // Saat formatı: HH:MM:SS
    saat = timeClient.getFormattedTime();
  } else {
    tarih = "1/1/2026";
    saat = "00:00:00";
  }

  timeClient.end();

  Serial.print("Tarih: ");
  Serial.println(tarih);
  Serial.print("Saat: ");
  Serial.println(saat);
}
/*Tarih saat işlemleri bitis*/

bool DatayiGonder(String cihazId, String fircaId) {
  if (WiFi.status() == WL_CONNECTED) {
    if (supabase_url.isEmpty() || supabase_key.isEmpty()) {
      Serial.println("Supabase URL veya API Key tanımlı değil!");
      return false;
    }

    printLocalTime();

    Serial.print("[Supabase] Free Heap: ");
    Serial.print(ESP.getFreeHeap());
    Serial.print(" | Max Block: ");
    Serial.println(ESP.getMaxAllocHeap());

    WiFiClientSecure *client = new WiFiClientSecure();
    if (!client) {
      Serial.println("[Supabase] Bellek ayrilamadi (WiFiClientSecure)!");
      return false;
    }
    client->setInsecure(); // Sertifika doğrulama kaynaklı TLS bellek/reset sorununu önler
    client->setHandshakeTimeout(10);

    HTTPClient http;
    String url = supabase_url + "/rest/v1/fircalama_kayitlari";

    http.setTimeout(10000);
    if (!http.begin(*client, url)) {
      Serial.println("[Supabase] HTTP baglantisi baslatilamadi!");
      delete client;
      return false;
    }

    http.addHeader("Content-Type", "application/json");
    http.addHeader("apikey", supabase_key);
    http.addHeader("Authorization", "Bearer " + supabase_key);
    http.addHeader("Prefer",
                   "return=minimal"); // Cevap olarak body dönmesini istemiyoruz

    // JSON Payload oluşturma (Manuel string birleştirme ile)
    String jsonPayload = "{";
    jsonPayload += "\"cihaz_id\":\"" + cihazId + "\",";
    jsonPayload += "\"firca_id\":\"" + fircaId + "\",";
    jsonPayload += "\"tarih\":\"" + tarih + "\",";
    jsonPayload += "\"saat\":\"" + saat + "\"";
    jsonPayload += "}";

    Serial.println("Supabase'e gönderiliyor: " + jsonPayload);

    int httpResponseCode = http.POST(jsonPayload);

    if (httpResponseCode > 0) {
      Serial.print("HTTP Response code: ");
      Serial.println(httpResponseCode);
      http.end();
      delete client;
      return (httpResponseCode == 200 || httpResponseCode == 201);
    } else {
      Serial.print("Error code: ");
      Serial.println(httpResponseCode);
      http.end();
      delete client;
      return false;
    }
  } else {
    Serial.println("WiFi Bağlı değil!");
    return false;
  }
}

#endif