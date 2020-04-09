#define FASTLED_ESP8266_NODEMCU_PIN_ORDER
#include <FastLED.h>
#include <ESP8266WiFi.h>
#include <WiFiUdp.h>
#include "secret.h"

#define TYPE 0
#define SEQ 1
#define BRIGHTNESS 2

#define PROTOCOL_SKIP 3

#define NUM_LEDS 64
#define LED_DATA_PIN 5

// debug
#define DEBUG 0
#define BLUE_PIN 2

// message types
#define LED_STRIP 1

const char* ssid     = SSID;
const char* password = PASS;
const unsigned int localUdpPort = 4210;
const unsigned short int offset = 2;

unsigned int tick_count = 0;
int seq = 0;
WiFiUDP Udp;
char incomingPacket[255];
CRGB leds[NUM_LEDS];

void connect() {
  if (WiFi.status() != WL_CONNECTED) {
    Serial.printf("Connecting to: %s\n", ssid);
    WiFi.begin(ssid, password);
    while (WiFi.status() != WL_CONNECTED) {
      delay(500);
      Serial.print(".");
    }
    Serial.printf("\nWiFi connected\n");
  }
  // Serial.printf("IP address: %s", WiFi.localIP());
}

void setup() {
  Serial.begin(115200);
  connect();
  Udp.begin(localUdpPort);
  FastLED.addLeds<WS2811, LED_DATA_PIN, BRG>(leds, NUM_LEDS);
  FastLED.setBrightness(50);
  pinMode(BLUE_PIN,    OUTPUT);
}

void try_connect() {
  tick_count++;
  if (tick_count > 1500000) {
    connect();
    tick_count = 0;
  }
}

void get_udp() {
  int packetSize = Udp.parsePacket();
  if (packetSize) {
    // Serial.printf("Received %d bytes from %s, port %d\n", packetSize, Udp.remoteIP().toString().c_str(), Udp.remotePort());
    int len = Udp.read(incomingPacket, 255);
    if (len > 0) { incomingPacket[len] = 0;	}
    // Serial.printf("UDP packet contents: %s\n", incomingPacket);
    do_led();
  }
}

int skip() {
  if (incomingPacket[SEQ] <= seq) {
    // here we probably rolled back to 0 set seq to 0 and  dont skip
    if (incomingPacket[SEQ] == 0 || ((seq - incomingPacket[SEQ]) > 20)) { seq = 0; return 0; }
    Serial.printf("seq is %d and incoming is %d, skipping\n", seq, incomingPacket[SEQ]);
    return 1;
  }
  return 0;
}

void do_led () {

#if DEBUG
  Serial.printf("sequence byte: %d\n", incomingPacket[0]);
#endif

  if (skip()) { return; }

  seq = incomingPacket[SEQ];
  FastLED.setBrightness(incomingPacket[BRIGHTNESS]);

  for (int i = 0; i < NUM_LEDS; i++) {
    leds[i].red   = incomingPacket[(i * 3) + PROTOCOL_SKIP + 0];
    leds[i].green = incomingPacket[(i * 3) + PROTOCOL_SKIP + 1];
    leds[i].blue  = incomingPacket[(i * 3) + PROTOCOL_SKIP + 2];
#if DEBUG
    Serial.println(leds[i]);
#endif
  }
  FastLED.show();
}

void loop () {
  get_udp();
  try_connect();
}
