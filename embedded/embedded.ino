#include <FastLED.h>
#include "WiFi.h"
#include <WiFiUdp.h>

#include "secret.h"

#define TYPE 0
#define SEQ 1
#define BRIGHTNESS 2

#define PROTOCOL_SKIP 3

#define NUM_LEDS 67
#define LED_DATA_PIN D0

// for fan control
// #define LIGHT_PIN   3
// #define FAN_ON_PIN  5
// #define FAN_OFF_PIN 4

// debug
#define DEBUG 0
#define BLUE_PIN LED_BUILTIN

// message types
#define LED_STRIP 1
#define FAN 99

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
    WiFi.mode(WIFI_STA);
    WiFi.begin(ssid, password);
    while (WiFi.status() != WL_CONNECTED) {
      delay(500);
      Serial.print(".");
    }
    Serial.printf("\nWiFi connected\n");
    Serial.printf(WiFi.localIP().toString().c_str());
  }
}

void setup() {
  Serial.begin(115200);
  connect();
  Udp.begin(localUdpPort);
  FastLED.addLeds<WS2811, LED_DATA_PIN, BRG>(leds, NUM_LEDS);
  FastLED.setBrightness(50);
  // pinMode(LIGHT_PIN,   OUTPUT);
  // pinMode(FAN_ON_PIN,  OUTPUT);
  // pinMode(FAN_OFF_PIN, OUTPUT);
  // pinMode(BLUE_PIN,    OUTPUT);
  pinMode(LED_BUILTIN, OUTPUT);
  pinMode(WIFI_ENABLE, OUTPUT); // pinMode(3, OUTPUT);
  digitalWrite(WIFI_ENABLE, LOW); // digitalWrite(3, LOW); // Activate RF switch control
  // digitalWrite(LED_BUILTIN, HIGH);
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
  // digitalWrite(LED_BUILTIN, LOW);
  // Serial.printf(".");
  if (packetSize) {
    Serial.printf("\nReceived %d bytes from %s, port %d\n", packetSize, Udp.remoteIP().toString().c_str(), Udp.remotePort());
    int len = Udp.read(incomingPacket, 255);
    if (len > 0) {
      incomingPacket[len] = 0;
    }
    // Serial.printf("UDP packet contents: %s\n", incomingPacket);
    // digitalWrite(LED_BUILTIN, HIGH);
    do_led();
  }
}

int skip() {
  if (incomingPacket[SEQ] <= seq) {
    // here we probably rolled back to 0 set seq to 0 and  dont skip
    if (incomingPacket[SEQ] == 0 || ((seq - incomingPacket[SEQ]) > 20)) {
      seq = 0;
      return 0;
    }
    Serial.printf("seq is %d and incoming is %d, skipping\n", seq, incomingPacket[SEQ]);
    return 1;
  }
  return 0;
}

void remote_press (int pin) {
  digitalWrite(pin, HIGH);
  delay(500);
  digitalWrite(pin, LOW);
}

void do_led () {
   Serial.printf("type: %d, sequence byte: %d\n", incomingPacket[TYPE], incomingPacket[SEQ]);
  if (incomingPacket[TYPE] == FAN) {
    // do_fan();
    return;
  }

#if DEBUG
  Serial.printf("sequence byte: %d\n", incomingPacket[SEQ]);
#endif

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
  delay(10);
}