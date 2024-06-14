# Nova control software documentation

The Nova control software is a Java application that controls the Nova hardware by directly sending ethernet frames to the hardware using [jnetpcap](https://github.com/slytechs-repos/jnetpcap-wrapper). In addition to controlling the connected Nova voxel modules (up to 10x10 modules, where as each module contains 5x5x10 LED voxels), the Nova control software provides a web interface running on the default interface on port 80 to control playback and content parameters.

## Development setup

### Nova server

The Nova server project is a Java project and can easily be imported into IDEs like Visual Studio Code (preferred) or Eclipse for development. Jar files can be built from command line using Maven (`mvn install`). The application launches via `NovaControl.main()`, and and creates `settings.conf` containing default settings, that can later be edited by the user.

Requirements:

- JDK 22 or later
- Maven or IDE with Maven support (preferred IDE is Visual Studio Code)

**Important:** Currently, the sources for jnetcap are included in the source tree. Once jnetcap for JDK 22 or later becomes available on Maven Central, these sources will be removed.

### Web app

The project includes a web app based on React / Material UI. The built and bundled app is included in the source repository at `src/main/resources/www`. Its sources are located at `src/main/webapp`. For development, you will need Node, and can the proceed as usual using `npm install`. As bundler, Vite is used, and you can use `npm run dev` to run the app in dev mode, and `npm run build` to build and copy the bundle to `src/resources/www`.

## Software configuration

The configuration is stored in `settings.conf` (which is automatically created on first start). Depending on your setup, edit the file and adjust the following parameters:

- `port`: The port the UI web server will be listening on. Default if omitted is `80`.
- `ethernet_interface`: The ethernet interface used for communicating with Nova. Default is `eth0`.
- `address_<X>_<Y>`: The address (as configured by jumpers on the Nova board, see below) of the module at module location (X,Y). Default is `address_0_0 = 1`.

A sample configuration could look like this:

```
port=80
ethernet_interface=eth1
address_0_0 = 1
address_0_1 = 5
address_0_2 = 9
address_0_3 = 13
address_1_0 = 2
address_1_1 = 6
address_1_2 = 10
address_1_3 = 14
address_2_0 = 3
address_2_1 = 7
address_2_2 = 11
address_2_3 = 15
address_3_0 = 4
address_3_1 = 8
address_3_2 = 12
address_3_3 = 16
...
```

All other parameters can be set using the web app.

## Hardware address configuration

The modules derive their MAC address from jumpers on the board. The jumpers encode the least significant byte of the MAC address. Refer to [Nova Jumper Configuration](nova_jumpers.jpg) in this documentation for an example

Generally, the server does not need to setup TCP/IP for the interface that communicates with the modules. However, the modules also set up their own IP address with the least significant 8 bits in the 192.168.1.0/24 subnet. This can be used for troubleshooting, e.g., for pinging the corresponding IP address to see if a module responds.

## Writing content extensions

The server loads content extensions as configured from a predefined package. Adding classes to this package makes them available to the server. A content extension must located in the `content` package and must inherit from the `Content` class. It needs to implement at least a constructor and the fillFrame() method. Below is the implementation of the `Content` class. For example:

```
package ch.bluecc.nova.content;

public class Sweep extends Content {
	/**
	 * Creates a content instance.
	 *
	 * @param name The name of the content.
	 * @param dimI The X-dimension.
	 * @param dimJ The Y-dimension.
	 * @param dimK The Z-dimension.
	 */
	public Sweep(int dimI, int dimJ, int dimK) {
		super("Sweep", dimI, dimJ, dimK);
	}

	/**
	 * Fill the frame.
	 */
	@Override
	public void fillFrame(float[] rgbFrame, double timeInSec) {
		final double dimK_1 = dimK - 1;
		// loop over all voxels
		for(int k = 0; k < dimK; k++) {
			double dk = k / dimK_1;
			// compute value based on time from a sine curve
			float v = (float)Math.abs(Math.sin(dk * Math.PI + timeInSec));
			for(int i = 0; i < dimI; i++)
				for(int j = 0; j < dimJ; j++)
					setVoxel(rgbFrame, i, j, k, v, v, v);
		}
	}
}
```

Be aware that `fillFrame()` must complete in 40ms, otherwise a frame underrun will occur. If the content is too complex to render in real-time, it can be written to a flat file of RGB voxels and played back with the ch.bluecc.nova.content.Movie class.

For additional examples and further details, refer to the source code.

## Troubleshooting

In case you have trouble getting your Nova up and running, you can to ping to your modules using the corresponding IP address. For this you need to configure TCP/IP for the Ethernet interface your Nova is connected to. Below steps apply to a Raspberry Pi setup, but doing this from macOS or another platform is analogous.

**Important:** likely, your home network runs on the 192.168.1.x network, you need to change this on your router, e.g. 192.168.2.x, since the 192.168.1.x network is used by the Nova hardware.

- Make sure your Nova is set to address 4 (see Nova Server documentation).
- From your machine, ssh to novahost.local

```
ssh pi@novahost.local
```

- Edit /etc/dhcpcd.conf, add:

```
interface eth0
static ip_address=192.168.1.130/24
```

- Reboot the Raspberry Pi & ssh to novahost.local again:

```
ssh pi@novahost.local
```

- You should be able to ping the Nova hardware via

```
ping 192.168.1.4
```

- If this is not the case, check your Nova address jumper settings again.
