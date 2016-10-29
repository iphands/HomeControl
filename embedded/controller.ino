#include <FastLED.h>
#include <ESP8266WiFi.h>
#include <WiFiUdp.h>
#include "secret.h"

#define TYPE 0
#define SEQ 1
#define BRIGHTNESS 2

#define PROTOCOL_SKIP 3

#define NUM_LEDS 80
#define LED_DATA_PIN 14

// for fan control
#define LIGHT_PIN   15
#define FAN_ON_PIN  5
#define FAN_OFF_PIN 4

// debug
#define DEBUG 0
#define BLUE_PIN 2

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
  pinMode(LIGHT_PIN,   OUTPUT);
  pinMode(FAN_ON_PIN,  OUTPUT);
  pinMode(FAN_OFF_PIN, OUTPUT);
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

void remote_press (int pin) {
  digitalWrite(pin, HIGH);
  delay(500);
  digitalWrite(pin, LOW);
}

void do_fan () {
  Serial.printf("sequence byte: %d %d\n", incomingPacket[0], incomingPacket[1]);
  switch (incomingPacket[1]) {
  case 97:
	Serial.println("pressing fan on button");
	remote_press(FAN_ON_PIN);
	break;
  case 98:
	Serial.println("pressing fan off button");
	remote_press(FAN_OFF_PIN);
	break;
  case 99:
	Serial.println("pressing fan light button");
	remote_press(LIGHT_PIN);
	break;
  case 100:
	Serial.println("blue pin...");
	remote_press(BLUE_PIN);
	break;
  }
}

void do_led () {

  if (incomingPacket[TYPE] == FAN) {
	do_fan();
	return;
  }

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
