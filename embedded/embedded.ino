#include <FastLED.h>
#include "WiFi.h"
#include <WiFiUdp.h>

#include "secret.h"

#define TYPE 0
#define SEQ 1
#define BRIGHTNESS 2
#define LED_LENGTH 3
#define ID 4
#define LED_START 5

#define LED_DATA_PIN D0

// for fan control
// #define LIGHT_PIN   3
// #define FAN_ON_PIN  5
// #define FAN_OFF_PIN 4

// debug
#define BLUE_PIN LED_BUILTIN

// message types
#define LED_STRIP 1
#define FAN 99

#define DEBUG 0

const char* ssid = SSID;
const char* password = PASS;
const unsigned int udp_port = 4210;
const unsigned short int offset = 2;

// const int DEVICE_ID = 0b00000001;
const int DEVICE_ID = 0b00000010;

unsigned int tick_count = 0;
int seq = 0;
int num_leds = 0;


WiFiUDP Udp;
char packet[255];
CRGB leds[255];

void connect() {
  // Serial.printf("DEBUG Connecting to: %s\n", ssid);
  if (WiFi.status() != WL_CONNECTED) {
    Serial.printf("Connecting to: %s\n", ssid);
    WiFi.mode(WIFI_STA);
    WiFi.begin(ssid, password);
    while (WiFi.status() != WL_CONNECTED) {
      delay(500);
      Serial.print(".");
    }
    Serial.printf("\nWiFi connected\n");
    Serial.printf(WiFi.localIP().toString().c_str());
    Serial.printf("\n");
  }
}

void setup() {
  Serial.begin(115200);
  connect();
  Udp.begin(udp_port);
  // FastLED.addLeds<WS2811, LED_DATA_PIN, BRG>(leds, num_leds);
  // FastLED.setBrightness(50);
  // pinMode(LIGHT_PIN,   OUTPUT);
  // pinMode(FAN_ON_PIN,  OUTPUT);
  // pinMode(FAN_OFF_PIN, OUTPUT);
  // pinMode(BLUE_PIN,    OUTPUT);
  pinMode(LED_BUILTIN, OUTPUT);
  pinMode(WIFI_ENABLE, OUTPUT);
  digitalWrite(WIFI_ENABLE, LOW);
  WiFi.setSleep(false);
}

void try_connect() {
  tick_count++;
  if (tick_count > 1500000) {
    Serial.printf("try_connect!\n");
    connect();
    tick_count = 0;
  }
}

void get_udp() {
  int packetSize = Udp.parsePacket();
  if (packetSize) {
    // Serial.printf("\nReceived %d bytes from %s, port %d\n", packetSize, Udp.remoteIP().toString().c_str(), Udp.remotePort());
    int len = Udp.read(packet, 255);
    if (len > 0) {
      packet[len] = 0;
    }
    // Serial.printf("UDP packet contents: %s\n", packet);
    digitalWrite(LED_BUILTIN, LOW);
    do_led();
  }
}

int skip() {
  if (packet[SEQ] <= seq) {
    // here we probably rolled back to 0 set seq to 0 and  dont skip
    if (packet[SEQ] == 0 || ((seq - packet[SEQ]) > 20)) {
      seq = 0;
      return 0;
    }
    Serial.printf("seq is %d and incoming is %d, skipping\n", seq, packet[SEQ]);
    return 1;
  }
  return 0;
}

void remote_press(int pin) {
  digitalWrite(pin, HIGH);
  delay(500);
  digitalWrite(pin, LOW);
}

void do_led() {
  // Serial.printf("type: %d, sequence byte: %d\n", packet[TYPE], packet[SEQ]);
  if (packet[TYPE] == FAN) {
    // do_fan();
    return;
  }

  //  Only continue if this is about me
  if ((DEVICE_ID != packet[ID])) {
    return;
  }

#if DEBUG
  Serial.printf("Got packet about me 0x%04lX == 0x%04lX\n", DEVICE_ID, packet[ID]);
#endif

  // First time we should set LED count
  if (num_leds <= 0) {
    num_leds = packet[LED_LENGTH];
    FastLED.addLeds<WS2811, LED_DATA_PIN, BRG>(leds, num_leds);
    FastLED.setBrightness(50);
  }

#if DEBUG
  Serial.printf("num_leds: %d\n", num_leds);
  Serial.printf("sequence byte: %d\n", packet[SEQ]);
#endif

  seq = packet[SEQ];
  FastLED.setBrightness(packet[BRIGHTNESS]);

  for (int i = 0; i < num_leds; i++) {
    leds[i].red = packet[(i * 3) + LED_START + 0];
    leds[i].green = packet[(i * 3) + LED_START + 1];
    leds[i].blue = packet[(i * 3) + LED_START + 2];
#if DEBUG
    Serial.printf("leds[%d] 0x%04lX 0x%04lX 0x%04lX\n", i, leds[i].red, leds[i].green, leds[i].blue);
#endif
  }
  FastLED.show();
}

void loop() {
  digitalWrite(LED_BUILTIN, HIGH);
  get_udp();
  try_connect();
}
