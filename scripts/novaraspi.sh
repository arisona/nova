#!/bin/sh
sleep 10
while true
do
sudo $JAVA_HOME/bin/java --enable-preview -Dorg.jnetpcap.libpcap.file=/usr/lib/aarch64-linux-gnu/libpcap.so.0.8 -jar target/novacontrol-1.0-SNAPSHOT.jar config/nova.properties
sleep 1
done
