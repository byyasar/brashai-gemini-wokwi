#ifndef SUPABASE_ISLEMLERI_H
#define SUPABASE_ISLEMLERI_H

#include "Arduino.h"
#include "config.h"
#include <HTTPClient.h>
#include <NTPClient.h>
#include <WiFi.h>
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
  // Saati güncellemek için birkaç kez dene (bağlantı hemen oturmayabilir)
  int retry = 0;
  while (!timeClient.update() && retry < 5) {
    timeClient.forceUpdate();
    retry++;
    delay(500);
  }

  time_t epochTime = timeClient.getEpochTime();
  struct tm *ptm = gmtime((time_t *)&epochTime);

  int currentYear = ptm->tm_year + 1900;
  int currentMonth = ptm->tm_mon + 1;
  int monthDay = ptm->tm_mday;

  // Tarih formatı: D/M/YYYY
  tarih =
      String(monthDay) + "/" + String(currentMonth) + "/" + String(currentYear);

  // Saat formatı: HH:MM:SS
  saat = timeClient.getFormattedTime();

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

    HTTPClient http;
    String url = supabase_url + "/rest/v1/fircalama_kayitlari";

    http.begin(url);
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
      return (httpResponseCode == 200 || httpResponseCode == 201);
    } else {
      Serial.print("Error code: ");
      Serial.println(httpResponseCode);
      http.end();
      return false;
    }
  } else {
    Serial.println("WiFi Bağlı değil!");
    return false;
  }
}

#endif