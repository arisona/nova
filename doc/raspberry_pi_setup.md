# Nova Raspberry Pi setup

This document provides step-by-step instructions to set up a Raspberry Pi in headless mode with the Nova software and integrate it into a home WLAN environment.

Basic Linux console experience is assumed. To edit files use either vi or nano. For details to configure a Raspberry Pi, refer to
headless setup from scratch for integration into home network: <https://www.raspberrypi.org/documentation/configuration/>.

## Step-by-step instructions

### Flash Raspberry Pi OS to SD Card

- Get Raspberry Pi Imager from <https://www.raspberrypi.com/software/>
- Choose your device (e.g., Raspberry Pi 4)
- Select OS: **Raspberry Pi OS Lite (64-bit)** (use latest, currently Debian Bookworm)
- Set up initial configuration before writing: host name, user/password, WLAN, timezone, enable SSH access. **Important:** make sure to get this configuration right, otherwise you will not be able to connect to your Raspberry Pi after you boot it for the first time. For the rest of this document, `nova` is assumed as host name -- feel free to use another name of your choice.
- Flash image to SD Card
- Put SD Card into Raspberry Pi

### Plugin your Raspberry Pi and wait until boot is complete

- From your machine, ssh to nova.local (or what ever hostname / username you have set):

```
ssh pi@nova.local
```

- In case hostname cannot be resolved, find the Raspberry Pi's IP address on your router and connect with ssh using the IP address.

- Run the Raspberry Pi configuration utility. Optional: this step allows you to change additional configuration parameters as required by your home setup. You can also use this in case you later move your Raspberry Pi to a different WLAN or if the WLAN password changes.

```
sudo raspi-config
```

### Update and install software

- Update the Raspberry Pi:

```
sudo apt update
sudo apt full-upgrade
sudo reboot
```

- Install required software:

```
sudo apt-get install git libcap0.8 maven
```

### Install OpenJDK 22 or later

Note: once OpenJDK 22 or later becomes available via apt-get, you can install the package via apt-get, and skip to the next section.

- Get latest OpenJDK package via wget: go to https://jdk.java.net, select JDK 22 or later ("Ready for Use"), and copy link to Linux/AArch64 `.tar.gz` package.
- In terminal on your Raspberry Pi, issue commands (make sure to update latest link and package name):

```
wget https://download.java.net/java/GA/jdk22.0.1/c7ec1332f7bb44aeba2eb341ae18aca4/8/GPL/openjdk-22.0.1_linux-aarch64_bin.tar.gz
tar xzf openjdk-22.0.1_linux-aarch64_bin.tar.gz
```

- This will result in your JDK being unpacked in your home directory at `/home/pi/jdk-22.0.1` You will need this path later for the automatic startup. Again, the exact path will be different for later JDK versions.

### Nova software setup and configuration

- Get the Nova control software release build and the launch script:

```
cd /home/pi
wget https://github.com/arisona/nova/releases/download/release_2_0_0/novacontrol-2.0.0-RELEASE.jar
wget https://github.com/arisona/nova/releases/download/release_2_0_0/novaraspi.sh
```

- Alternatively, if you want to build from source, here is how to get and compile the Nova code:

```
cd /home/pi
git clone https://github.com/arisona/nova.git
cd nova
export JAVA_HOME=/home/pi/jdk-22.0.1/
mvn install
```

### Configure startup script and reboot

- Edit `/etc/rc.local` (e.g. `sudo nano /etc/rc.local`), add (before `exit 0`):

```
sh /home/pi/novaraspi.sh > /dev/null 2>&1 &
```

- Plug in Nova via ethernet and reboot

```
sudo shutdown -r now
```

- After about a minute, the Nova should go on
- Connect to web interface via your web browser: http://nova.local

### Changing default options

By default, the control software assumes `eth0` as ethernet interface to communicate, with one module connected and its jumper set to address 1. To change these settings, connect using the web app and adjust interface or module 0 address as needed.

If you have multiple modules, the configuration need to be manually edited. After launching the Nova server once, you will find a file `settings.conf` in your home directory. Refer to the [settings example](doc/settings_example.txt) to find instructions to configure multiple modules.
