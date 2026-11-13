#include "Arduino.h"
#include "Delay.h"
#include "LedControl.h"
#include <Wire.h>

/*1- Kum saati baslangıcı*/
#define PIN_DATAIN 14 // SCK-din
#define PIN_CLK 13    // MOSI-clk
#define PIN_LOAD 15   // SS-cs
/*1- Kum saati bitis*/

/*2- Kum saati baslangıcı*/
const int xOrigin = 64;
const int yOrigin = 32;
float x, y;
#define MATRIX_A 0
#define MATRIX_B 1
#define ACC_THRESHOLD_LOW 0
#define ACC_THRESHOLD_HIGH 0
unsigned long lastDebounce = 0;
bool calis = false;
#define ROTATION_OFFSET 90

int getGravity() { return 0; }
// in milliseconds
#define DEBOUNCE_THRESHOLD 500
#define DELAY_FRAME 80
unsigned long particleDelayMs =
    2000; // Default 2 minutes (120s / 60 = 2s = 2000ms)

void setHourglassDuration(int seconds) {
  particleDelayMs = (seconds * 1000) / 60;
}
int gravity;
LedControl lc = LedControl(PIN_DATAIN, PIN_CLK, PIN_LOAD, 2);
NonBlockDelay d;
int resetCounter = 0;

coord getDown(int x, int y) {
  coord xy;
  xy.x = x - 1;
  xy.y = y + 1;
  return xy;
}
coord getLeft(int x, int y) {
  coord xy;
  xy.x = x - 1;
  xy.y = y;
  return xy;
}
coord getRight(int x, int y) {
  coord xy;
  xy.x = x;
  xy.y = y + 1;
  return xy;
}
bool canGoLeft(int addr, int x, int y) {
  if (x == 0)
    return false;                        // not available
  return !lc.getXY(addr, getLeft(x, y)); // you can go there if this is empty
}
bool canGoRight(int addr, int x, int y) {
  if (y == 7)
    return false;                         // not available
  return !lc.getXY(addr, getRight(x, y)); // you can go there if this is empty
}
bool canGoDown(int addr, int x, int y) {
  if (y == 7)
    return false; // not available
  if (x == 0)
    return false; // not available
  if (!canGoLeft(addr, x, y))
    return false;
  if (!canGoRight(addr, x, y))
    return false;
  return !lc.getXY(addr, getDown(x, y)); // you can go there if this is empty
}
void goDown(int addr, int x, int y) {
  lc.setXY(addr, x, y, false);
  lc.setXY(addr, getDown(x, y), true);
}
void goLeft(int addr, int x, int y) {
  lc.setXY(addr, x, y, false);
  lc.setXY(addr, getLeft(x, y), true);
}
void goRight(int addr, int x, int y) {
  lc.setXY(addr, x, y, false);
  lc.setXY(addr, getRight(x, y), true);
}
int countParticles(int addr) {
  int c = 0;
  for (byte y = 0; y < 8; y++) {
    for (byte x = 0; x < 8; x++) {
      if (lc.getXY(addr, x, y)) {
        c++;
      }
    }
  }
  return c;
}
bool moveParticle(int addr, int x, int y) {
  if (!lc.getXY(addr, x, y)) {
    return false;
  }

  bool can_GoLeft = canGoLeft(addr, x, y);
  bool can_GoRight = canGoRight(addr, x, y);

  if (!can_GoLeft && !can_GoRight) {
    return false; // we're stuck
  }

  bool can_GoDown = canGoDown(addr, x, y);

  if (can_GoDown) {
    goDown(addr, x, y);
  } else if (can_GoLeft && !can_GoRight) {
    goLeft(addr, x, y);
  } else if (can_GoRight && !can_GoLeft) {
    goRight(addr, x, y);
  } else if (random(2) == 1) { // we can go left and right, but not down
    goLeft(addr, x, y);
  } else {
    goRight(addr, x, y);
  }
  return true;
}
void fill(int addr, int maxcount) {
  int n = 8;
  byte x, y;
  int count = 0;
  for (byte slice = 0; slice < 2 * n - 1; ++slice) {
    byte z = slice < n ? 0 : slice - n + 1;
    for (byte j = z; j <= slice - z; ++j) {
      y = 7 - j;
      x = (slice - j);
      lc.setXY(addr, x, y, (++count <= maxcount));
    }
  }
}
int getTopMatrix() { return (getGravity() == 0) ? MATRIX_A : MATRIX_B; }
int getBottomMatrix() { return (getGravity() != 0) ? MATRIX_A : MATRIX_B; }
void resetTime() {
  for (byte i = 0; i < 2; i++) {
    lc.clearDisplay(i);
  }
  fill(getTopMatrix(), 60);
  d.Delay(particleDelayMs);
}
bool updateMatrix() {
  int n = 8;
  bool somethingMoved = false;
  byte x, y;
  bool direction;
  for (byte slice = 0; slice < 2 * n - 1; ++slice) {
    direction =
        (random(2) ==
         1); // randomize if we scan from left to right or from right to left,
             // so the grain doesn't always fall the same direction
    byte z = slice < n ? 0 : slice - n + 1;
    for (byte j = z; j <= slice - z; ++j) {
      y = direction ? (7 - j) : (7 - (slice - j));
      x = direction ? (slice - j) : j;
      // for (byte d=0; d<2; d++) { lc.invertXY(0, x, y); delay(50); }
      if (moveParticle(MATRIX_B, x, y)) {
        somethingMoved = true;
      };
      if (moveParticle(MATRIX_A, x, y)) {
        somethingMoved = true;
      }
    }
  }
  return somethingMoved;
}
boolean dropParticle() {
  if (d.Timeout()) {
    d.Delay(particleDelayMs);
    if (gravity == 0 || gravity == 180) {
      if ((lc.getRawXY(MATRIX_A, 0, 0) && !lc.getRawXY(MATRIX_B, 7, 7)) ||
          (!lc.getRawXY(MATRIX_A, 0, 0) && lc.getRawXY(MATRIX_B, 7, 7))) {
        // for (byte d=0; d<8; d++) { lc.invertXY(0, 0, 7); delay(50); }
        lc.invertRawXY(MATRIX_A, 0, 0);
        lc.invertRawXY(MATRIX_B, 7, 7);
        return true;
      }
    }
  }
  return false;
}
void resetCheck() {
  /*int z = analogRead(A3);
  if (z > ACC_THRESHOLD_HIGH || z < ACC_THRESHOLD_LOW) {
    resetCounter++;
    Serial.println(resetCounter);
  } else {
    resetCounter = 0;
  }*/
  if (resetCounter > 20) {
    Serial.println("RESET!");
    resetTime();
    resetCounter = 0;
  }
}

// ---------------- Wi-Fi Dot Matris Görsel Bildirimleri ----------------
// Kum saati kasasındaki 45° elmas (baklava) montaj açısına göre optimize edilmiş çizimler

void matrisCiz(int addr, const byte bitmap[8]) {
  for (int row = 0; row < 8; row++) {
    lc.setRow(addr, row, bitmap[row]);
  }
}

// Wi-Fi Animasyon Kareleri (45° Diyagonal / Elmas Açı)
const byte WIFI_FRAME_1[8] = {
  0b11000000,
  0b11000000,
  0b00010000,
  0b00110000,
  0b00000000,
  0b00000000,
  0b00000000,
  0b00000000
};

const byte WIFI_FRAME_2[8] = {
  0b11000000,
  0b11010100,
  0b00010100,
  0b01110100,
  0b00000100,
  0b01111100,
  0b00000000,
  0b00000000
};

const byte WIFI_FRAME_3[8] = {
  0b11000000,
  0b11010101,
  0b00010101,
  0b01110101,
  0b00000101,
  0b01111101,
  0b00000001,
  0b01111111
};

// Onay Simgesi (Elmas kasada tam yukarı bakan, 180° çevrilmiş Tik ✔)
const byte ICON_CHECKMARK[8] = {
  0b00000000,
  0b01110000,
  0b01111000,
  0b01101100,
  0b01100110,
  0b01100010,
  0b01100000,
  0b00000000
};

// Hata Simgesi (Elmas Kasada Çarpı ✖ Görünen Şekil)
const byte ICON_CROSS[8] = {
  0b00011000,
  0b00011000,
  0b00011000,
  0b11111111,
  0b11111111,
  0b00011000,
  0b00011000,
  0b00011000
};

// Wi-Fi Bağlanıyor animasyon adımı (Sadece A displayinde dalga gösterir, B displayi temiz kalır)
void dotMatrisWifiAnimasyonu(int frame) {
  const byte* f = (frame % 3 == 0) ? WIFI_FRAME_1 : (frame % 3 == 1 ? WIFI_FRAME_2 : WIFI_FRAME_3);
  matrisCiz(MATRIX_A, f);
  lc.clearDisplay(MATRIX_B);
}

// Wi-Fi Başarıyla Bağlandı -> A displayinde Wi-Fi dalgası, B displayinde animasyonlu TİK ✔
void dotMatrisWifiBasarili() {
  lc.clearDisplay(MATRIX_A);
  lc.clearDisplay(MATRIX_B);
  matrisCiz(MATRIX_A, WIFI_FRAME_3);   // Üst matris: Wi-Fi dalgası

  // 180° çevrilmiş Tik animasyonu: B displayinde alttan yukarıya doğru adım adım çizilir
const byte animSteps[6][8] = {
  { 0b00000000, 0b00000110, 0b00001100, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000 },
  { 0b00000000, 0b00011110, 0b00111100, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000 },
  { 0b00000000, 0b01111110, 0b01111100, 0b01000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000 },
  { 0b00000000, 0b01111110, 0b01111100, 0b01100000, 0b01100000, 0b01000000, 0b00000000, 0b00000000 },
  { 0b00000000, 0b01111100, 0b01111000, 0b01100000, 0b01100000, 0b01100000, 0b00100000, 0b00000000 },
  { 0b00000000, 0b01111100, 0b01111000, 0b01100000, 0b01100000, 0b01100000, 0b00100000, 0b00000000 }
};

for (int step = 0; step < 6; step++) {
  matrisCiz(MATRIX_B, animSteps[step]);
  delay(100);
}
  delay(1400);                          // Ekranda sabit tut
  lc.clearDisplay(MATRIX_A);
  lc.clearDisplay(MATRIX_B);
}

// Wi-Fi Başarısız -> B displayinde Çarpı ✖ (A displayi temiz)
void dotMatrisWifiBasarisiz() {
  lc.clearDisplay(MATRIX_A);
  lc.clearDisplay(MATRIX_B);
  matrisCiz(MATRIX_B, ICON_CROSS);
  delay(1400);
  lc.clearDisplay(MATRIX_A);
  lc.clearDisplay(MATRIX_B);
}

// ---------------- Supabase Veri Gönderim Animasyonları ----------------
// Supabase'e veri gönderilirken A displayinde gösterilecek bekleme sembolü
const byte wait[8] = {
  0b00110000,
  0b01101000,
  0b11100100,
  0b10010010,
  0b01001111,
  0b00101111,
  0b00011110,
  0b00001100
};

// A displayinde bekleme sembolünü gösterir
void dotMatrisWaitGoster() {
  lc.shutdown(MATRIX_A, false);
  lc.setIntensity(MATRIX_A, 2);
  matrisCiz(MATRIX_A, wait);
}

// Bir noktanın daire etrafında dönmesi (Dönen Nokta / Spinner - 8 adım)
const byte SPINNER_FRAME[8][8] = {
  // 0: Üst merkez
  { 0, 0b00011000, 0, 0, 0, 0, 0, 0 },
  // 1: Üst-sağ
  { 0, 0, 0b00000100, 0, 0, 0, 0, 0 },
  // 2: Sağ merkez
  { 0, 0, 0, 0b00000010, 0b00000010, 0, 0, 0 },
  // 3: Alt-sağ
  { 0, 0, 0, 0, 0, 0b00000100, 0, 0 },
  // 4: Alt merkez
  { 0, 0, 0, 0, 0, 0, 0b00011000, 0 },
  // 5: Alt-sol
  { 0, 0, 0, 0, 0, 0b00100000, 0, 0 },
  // 6: Sol merkez
  { 0, 0, 0, 0b01000000, 0b01000000, 0, 0, 0 },
  // 7: Üst-sol
  { 0, 0, 0b00100000, 0, 0, 0, 0, 0 }
};

// Supabase'e veri gönderilirken A displayinde wait sembolü, B displayinde (alt matris) daire etrafında dönen nokta
void dotMatrisVeriGonderimAnimasyonu(int step) {
  int phase = step % 8;
  matrisCiz(MATRIX_A, wait);
  matrisCiz(MATRIX_B, SPINNER_FRAME[phase]);
}

// Veri gönderim sonucu: Başarılıysa animasyonlu Tik ✔, Başarısızsa Çarpı ✖
void dotMatrisVeriGonderimSonuc(bool basarili) {
  lc.clearDisplay(MATRIX_A);
  lc.clearDisplay(MATRIX_B);
  if (basarili) {
const byte animSteps[6][8] = {
  { 0b00000000, 0b00000110, 0b00001100, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000 },
  { 0b00000000, 0b00011110, 0b00111100, 0b00000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000 },
  { 0b00000000, 0b01111110, 0b01111100, 0b01000000, 0b00000000, 0b00000000, 0b00000000, 0b00000000 },
  { 0b00000000, 0b01111110, 0b01111100, 0b01100000, 0b01100000, 0b01000000, 0b00000000, 0b00000000 },
  { 0b00000000, 0b01111100, 0b01111000, 0b01100000, 0b01100000, 0b01100000, 0b00100000, 0b00000000 },
  { 0b00000000, 0b01111100, 0b01111000, 0b01100000, 0b01100000, 0b01100000, 0b00100000, 0b00000000 }
};
for (int step = 0; step < 6; step++) {
  matrisCiz(MATRIX_B, animSteps[step]);
  delay(100);
}
    delay(1200);
  } else {
    matrisCiz(MATRIX_B, ICON_CROSS);
    delay(1200);
  }

  lc.clearDisplay(MATRIX_A);
  lc.clearDisplay(MATRIX_B);
}

// ---------------- 1-5 Fırça Rakam Bitmapleri (90° Sağa Döndürülmüş) ----------------
const byte DIGIT_NUMBERS[6][8] = {
  { 0, 0, 0, 0, 0, 0, 0, 0 }, // 0: Boş
  // 1
  {
    0b00000000,
    0b10000100,
    0b10000110,
    0b11111111,
    0b11111111,
    0b10000000,
    0b10000000,
    0b00000000
  },
  // 2
  {
    0b00000000,
    0b11000010,
    0b11100011,
    0b10110001,
    0b10011001,
    0b10001111,
    0b10000110,
    0b00000000
  },
  // 3
  {
    0b00000000,
    0b01000010,
    0b11000011,
    0b10001001,
    0b10001001,
    0b11110111,
    0b01110110,
    0b00000000
  },
  // 4
  {
    0b00110000,
    0b00111000,
    0b00101100,
    0b00100110,
    0b11111111,
    0b11111111,
    0b00100000,
    0b00000000
  },
  // 5
  {
    0b00000000,
    0b01001111,
    0b11001111,
    0b10001001,
    0b10001001,
    0b11111001,
    0b01110001,
    0b00000000
  }
};

// A displayinde kaldırılan fırçanın numarasını (1-5) gösterir
void dotMatrisRakamGoster(int fircaNo) {
  if (fircaNo >= 1 && fircaNo <= 5) {
    lc.shutdown(MATRIX_A, false);
    lc.setIntensity(MATRIX_A, 2);
    matrisCiz(MATRIX_A, DIGIT_NUMBERS[fircaNo]);
  } else {
    lc.clearDisplay(MATRIX_A);
  }
}

// ---------------- Gülen Yüz (Smiley Face) Bitmap ----------------
const byte ICON_SMILEY[8] = {
    0b00111100,
  0b01000010,
  0b10011001,
  0b10100001,
  0b10100101,
  0b10001001,
  0b01000010,
  0b00111100
};

// A displayinde gülen yüz sembolünü gösterir
void dotMatrisGulenYuzGoster() {
  lc.shutdown(MATRIX_A, false);
  lc.setIntensity(MATRIX_A, 2);
  matrisCiz(MATRIX_A, ICON_SMILEY);
}

// ---------------- Pervane (Fan) Animasyon Kareleri ----------------
const byte PROPELLER_FRAME[4][8] = {
  // 0: Artı (+)
  {
    0b00011000,
    0b00011000,
    0b00011000,
    0b11111111,
    0b11111111,
    0b00011000,
    0b00011000,
    0b00011000
  },
  // 1: Eğimli (22.5°)
  {
    0b00001100,
    0b00001100,
    0b10011000,
    0b11111110,
    0b01111111,
    0b00011001,
    0b00110000,
    0b00110000
  },
  // 2: Çapraz (X - 45°)
  {
    0b11000011,
    0b01100110,
    0b00111100,
    0b00011000,
    0b00011000,
    0b00111100,
    0b01100110,
    0b11000011
  },
  // 3: Eğimli (67.5°)
  {
    0b00110000,
    0b00110000,
    0b00011001,
    0b01111111,
    0b11111110,
    0b10011000,
    0b00001100,
    0b00001100
  }
};

// ---------------- Kurutma Sırasında B Displayinde Gülümseme Animasyon Kareleri (9 Adım) ----------------
const byte SMILE_ICON[9][8] = {
  { 0b00011000, 0b00110000, 0b01100000, 0b11000000, 0b10000000, 0b00000000, 0b00000000, 0b00000000 },
  { 0b00011000, 0b00111000, 0b01110000, 0b11000000, 0b11000000, 0b00000000, 0b00000000, 0b00000000 },
  { 0b00011000, 0b00111100, 0b01110000, 0b11010000, 0b11000000, 0b01000000, 0b00000000, 0b00000000 },
  { 0b00011000, 0b00111110, 0b01110000, 0b11010000, 0b11010000, 0b01000000, 0b01000000, 0b00000000 },
  { 0b00011000, 0b00111110, 0b01110010, 0b11010000, 0b11011000, 0b01000000, 0b01100000, 0b00000000 },
  { 0b00011000, 0b00111110, 0b01110010, 0b11010010, 0b11011100, 0b01000000, 0b01100000, 0b00000000 },
  { 0b00011000, 0b00111110, 0b01110010, 0b11010011, 0b11011100, 0b01000100, 0b01110000, 0b00010000 },
  { 0b00011000, 0b00111110, 0b01110010, 0b11010011, 0b11011101, 0b01000100, 0b01110100, 0b00010100 },
  { 0b00011000, 0b00111110, 0b01110010, 0b11010011, 0b11011101, 0b01000100, 0b01110100, 0b00010111 }
};

// Kurutma işlemi adımı: A displayinde pervane, B displayinde SMILE_ICON animasyonu
void dotMatrisKurutmaAdimi(int fanStep, int smileStep) {
  lc.shutdown(MATRIX_A, false);
  lc.setIntensity(MATRIX_A, 2);
  lc.shutdown(MATRIX_B, false);
  lc.setIntensity(MATRIX_B, 2);
  matrisCiz(MATRIX_A, PROPELLER_FRAME[fanStep % 4]);
  matrisCiz(MATRIX_B, SMILE_ICON[smileStep % 9]);
}

// Kurutma animasyonu: A displayinde pervane, B displayinde gülümseme animasyonu
void dotMatrisPervaneAdimi(int step) {
  dotMatrisKurutmaAdimi(step, step);
}

// ---------------- Müzik Notası Sallanma (Dans) Animasyon Kareleri (90° Sağa Döndürülmüş) ----------------
// Notanın hem alt tarafı (kafalar) hem üst kolları komple sağa-sola sallanır
const byte MUSIC_NOTE_FRAMES[4][8] = {
  { 0b01000000, 0b11100000, 0b01111110, 0b00000110, 0b01000110, 0b11100110, 0b01111110, 0b00000000 },
  { 0b00000000, 0b01000000, 0b11100000, 0b01111110, 0b00000110, 0b01000110, 0b11100110, 0b01111110 },
  { 0b01000000, 0b11100000, 0b01111110, 0b00000110, 0b01000110, 0b11100110, 0b01111110, 0b00000000 },
  { 0b11100000, 0b01111110, 0b00000110, 0b01000110, 0b11100110, 0b01111110, 0b00000000, 0b00000000 }
};

// B displayinde sağa sola sallanan müzik notası animasyonu adımı
void dotMatrisNotaSalla(int step) {
  lc.shutdown(MATRIX_B, false);
  lc.setIntensity(MATRIX_B, 2);
  matrisCiz(MATRIX_B, MUSIC_NOTE_FRAMES[step % 4]);
}

// B displayinde sabit müzik notası sembolünü (♫) gösterir
void dotMatrisNotaGoster() {
  dotMatrisNotaSalla(0);
}

/*2- Kum saati bitis*/