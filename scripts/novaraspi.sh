#!/bin/sh
export JAVA_HOME=/home/pi/jdk-23.0.1/
cd /home/pi
sleep 10
while true
do
sudo $JAVA_HOME/bin/java -Dorg.jnetpcap.libpcap.file=/usr/lib/aarch64-linux-gnu/libpcap.so.0.8 -jar novacontrol-2.1.0-RELEASE.jar
sleep 1
done