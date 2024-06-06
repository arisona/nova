package org.corebounce.nova;

import java.io.File;
import java.io.FileReader;
import java.io.FileWriter;
import java.io.IOException;
import java.util.ArrayList;
import java.util.BitSet;
import java.util.List;
import java.util.Properties;

public final class State {

  private static final String CONFIG_FILE = "settings.conf";

  private static final String CONFIG_KEY_ADDRESS = "address_";
  private static final String CONFIG_KEY_PORT = "port";

  private static final String CONFIG_KEY_ETHERNET_INTERFACE = "ethernet_interface";
  private static final String CONFIG_KEY_ENABLED_CONTENT = "enabled_content";
  private static final String CONFIG_KEY_SELECTED_CONTENT = "selected_duration";
  private static final String CONFIG_KEY_HUE = "hue";
  private static final String CONFIG_KEY_SATURATION = "saturation";
  private static final String CONFIG_KEY_BRIGHTNESS = "brightness";
  private static final String CONFIG_KEY_SPEED = "speed";
  private static final String CONFIG_KEY_FLIP_VERTICAL = "flip_vertical";
  private static final String CONFIG_KEY_CYCLE_DURATION = "cycle_duration";

  private static final int CONFIG_DEFAULT_ADDRESS = 1;
  private static final int CONFIG_DEFAULT_PORT = 80;
  private static final String CONFIG_DEFAULT_ETHERNET_INTERFACE = "eth0";
  private static final int CONFIG_DEFAULT_SELECTED_CONTENT = 0;
  private static final float CONFIG_DEFAULT_HUE = 0.5f;
  private static final float CONFIG_DEFAULT_SATURATION = 1.0f;
  private static final float CONFIG_DEFAULT_BRIGHTNESS = 0.5f;
  private static final float CONFIG_DEFAULT_SPEED = 0.5f;
  private static final boolean CONFIG_DEFAULT_FLIP_VERTICAL = false;
  private static final float CONFIG_DEFAULT_CYCLE_DURATION = 0;

  // immutable state set at startup
  private final int[][] moduleConfig = new int[100][100];
  private final int[] modulesFlat;
  private final int[] frameOffsets = new int[256];
  private final int dimI;
  private final int dimJ;

  private final int port;

  private final List<Content> availableContent = new ArrayList<>();

  // mutable state set at startup and by UI
  private int selectedContentIndex;
  private float hue;
  private float saturation;
  private float brightness;
  private float speed;

  private BitSet enabledContentIndices;
  private boolean flipVertical;
  private float cycleDuration;
  private String ethernetInterface;

  // internal state
  private final DMUXStatus[] status = new DMUXStatus[101];

  public State() {
    //// get config file
    var config = new Properties();
    try {
      config.load(new FileReader(new File(CONFIG_FILE)));
      Log.info("Using config file " + CONFIG_FILE);
    } catch (IOException t) {
      Log.info("Could not load config file " + CONFIG_FILE + ". Using defaults");
    }

    //// get module configuration
    var moduleDimI = 0;
    var moduleDimJ = 0;
    for (var i = 0; i < moduleConfig.length; i++) {
      for (var j = 0; j < moduleConfig[i].length; j++) {
        var addr = getString(config, CONFIG_KEY_ADDRESS + i + "_" + j, null);
        if (addr != null) {
          moduleConfig[i][j] = Integer.parseInt(addr.trim());
          moduleDimI = Math.max(moduleDimI, i + 1);
          moduleDimJ = Math.max(moduleDimJ, j + 1);
        }
      }
    }

    if (moduleDimI == 0 || moduleDimJ == 0) {
      // if no addresses are given, default to addr_0_0 = default
      moduleDimI = 1;
      moduleDimJ = 1;
      moduleConfig[0][0] = CONFIG_DEFAULT_ADDRESS;
    }

    var modules = new int[moduleDimI][moduleDimJ];
    for (var i = 0; i < moduleDimI; i++) {
      for (var j = 0; j < moduleDimJ; j++) {
        modules[i][j] = moduleConfig[i][j];
      }
    }
    dimI = modules.length * getModuleDimI();
    var tmp = new int[100];
    var count = 0;
    var maxJ = 0;
    for (var row : modules) {
      maxJ = Math.max(maxJ, row.length);
    }
    for (var j = 0; j < maxJ; j++) {
      for (var i = 0; i < modules.length; i++) {
        var m = modules[i][j];
        if (m > 0 && m < 101) {
          tmp[count++] = m;
          frameOffsets[m] = calcFrameOffset(modules, m);
        }
      }
    }
    this.dimJ = maxJ * getModuleDimJ();
    this.modulesFlat = new int[count];
    System.arraycopy(tmp, 0, this.modulesFlat, 0, count);

    //// get other settings
    port = getInt(config, CONFIG_KEY_PORT, CONFIG_DEFAULT_PORT);
    ethernetInterface = getString(config, CONFIG_KEY_ETHERNET_INTERFACE, CONFIG_DEFAULT_ETHERNET_INTERFACE);
    selectedContentIndex = getInt(config, CONFIG_KEY_SELECTED_CONTENT, CONFIG_DEFAULT_SELECTED_CONTENT);
    hue = getFloat(config, CONFIG_KEY_HUE, CONFIG_DEFAULT_HUE);
    saturation = getFloat(config, CONFIG_KEY_SATURATION, CONFIG_DEFAULT_SATURATION);
    brightness = getFloat(config, CONFIG_KEY_BRIGHTNESS, CONFIG_DEFAULT_BRIGHTNESS);
    speed = getFloat(config, CONFIG_KEY_SPEED, CONFIG_DEFAULT_SPEED);

    flipVertical = getBoolean(config, CONFIG_KEY_FLIP_VERTICAL, CONFIG_DEFAULT_FLIP_VERTICAL);
    cycleDuration = getFloat(config, CONFIG_KEY_CYCLE_DURATION, CONFIG_DEFAULT_CYCLE_DURATION);

    availableContent.addAll(Content.createContent(this));
    enabledContentIndices = getBitSet(config, CONFIG_KEY_ENABLED_CONTENT, availableContent.size(), true);

    var msg = String.format(
      "Set up Nova state with %dx%d modules (%dx%dx%d voxels), port %d, interface %s",
      moduleDimI,
      moduleDimJ,
      dimI,
      dimJ,
      getModuleDimK(),
      port,
      ethernetInterface
    );
    Log.info(msg);

    writeSettings();
  }

  public void restore() {
    Log.info("Restoring settings");
    if (getNumModules() == 1) moduleConfig[0][0] = CONFIG_DEFAULT_ADDRESS;

    ethernetInterface = CONFIG_DEFAULT_ETHERNET_INTERFACE;
    selectedContentIndex = CONFIG_DEFAULT_SELECTED_CONTENT;
    hue = CONFIG_DEFAULT_HUE;
    saturation = CONFIG_DEFAULT_SATURATION;
    brightness = CONFIG_DEFAULT_BRIGHTNESS;
    speed = CONFIG_DEFAULT_SPEED;

    flipVertical = CONFIG_DEFAULT_FLIP_VERTICAL;
    cycleDuration = CONFIG_DEFAULT_CYCLE_DURATION;

    enabledContentIndices = new BitSet(availableContent.size());
    enabledContentIndices.set(0, availableContent.size());
  }

  public int[] getModules() {
    return modulesFlat;
  }

  public int getNumModules() {
    return modulesFlat.length;
  }

  public int getFrameOffset(int m) {
    return frameOffsets[m];
  }

  public int getModuleDimI() {
    return 5;
  }

  public int getModuleDimJ() {
    return 5;
  }

  public int getModuleDimK() {
    return 10;
  }

  public int getDimI() {
    return dimI;
  }

  public int getDimJ() {
    return dimJ;
  }

  public int getDimK() {
    return getModuleDimK();
  }

  public int getPort() {
    return port;
  }

  public List<Content> getAvailableContent() {
    return availableContent;
  }

  public Content getContent(int index) {
    return availableContent.get(index % availableContent.size());
  }

  public int getSelectedContentIndex() {
    return selectedContentIndex;
  }

  public void setSelectedContentIndex(int selectedContentIndex) {
    this.selectedContentIndex = selectedContentIndex % availableContent.size();
  }

  public float getHue() {
    return hue;
  }

  public void setHue(float hue) {
    this.hue = hue;
  }

  public float getSaturation() {
    return saturation;
  }

  public void setSaturation(float saturation) {
    this.saturation = saturation;
  }

  public float getBrightness() {
    return brightness;
  }

  public void setBrightness(float brightness) {
    this.brightness = brightness;
  }

  public float getSpeed() {
    return speed;
  }

  public void setSpeed(float speed) {
    this.speed = speed;
  }

  public BitSet getEnabledContentIndices() {
    return enabledContentIndices;
  }

  public void setEnabledContentIndices(BitSet enabledContentIndices) {
    this.enabledContentIndices = enabledContentIndices;
  }

  public boolean isFlipVertical() {
    return flipVertical;
  }

  public void setFlipVertical(boolean flipVertical) {
    this.flipVertical = flipVertical;
  }

  public float getCycleDuration() {
    return cycleDuration;
  }

  public void setCycleDuration(float cycleDuration) {
    this.cycleDuration = cycleDuration;
  }

  public String getEthernetInterface() {
    return ethernetInterface;
  }

  public void setEthernetInterface(String ethernetInterface) {
    this.ethernetInterface = ethernetInterface;
  }

  public String getModule0Address() {
    return Integer.toString(moduleConfig[0][0]);
  }

  public void setModule0Address(String module0Address) {
    if (getNumModules() == 1) moduleConfig[0][0] = Integer.parseInt(module0Address);
    else Log.warning("Cannot set module 0 address for multiple modules");
  }

  public boolean isOperational() {
    return getNumOperational() > modulesFlat.length / 2;
  }

  public boolean isStatusOk() {
    if (NovaControlMain.get().getDevice().isDummy()) return false;
    return !isOperational();
  }

  public String getStatusMessage() {
    if (NovaControlMain.get().getDevice().isDummy()) return "Cannot connect using interface " + ethernetInterface;
    return "" + getNumOperational() + " of " + modulesFlat.length + " modules operational";
  }

  public int getNumOperational() {
    int result = 0;
    for (int m : modulesFlat) {
      if (status[m] != null && status[m].isOperational()) {
        result++;
      }
    }
    return result;
  }

  public void setDMUXStatus(DMUXStatus status) {
    this.status[status.ipAddr & 0xFF] = status;
  }

  public void writeSettings() {
    var s = new StringBuilder();
    s.append("# Nova settings\n");
    for (var i = 0; i < moduleConfig.length; i++) {
      for (var j = 0; j < moduleConfig[i].length; j++) {
        if (moduleConfig[i][j] == 0) continue;
        s.append(CONFIG_KEY_ADDRESS).append(i).append("_").append(j).append("=");
        s.append(moduleConfig[i][j]).append("\n");
      }
    }
    s.append(CONFIG_KEY_PORT).append("=").append(port).append("\n");
    s.append(CONFIG_KEY_ETHERNET_INTERFACE).append("=").append(ethernetInterface).append("\n");
    s.append(CONFIG_KEY_SELECTED_CONTENT).append("=").append(selectedContentIndex).append("\n");
    s.append(CONFIG_KEY_HUE).append("=").append(hue).append("\n");
    s.append(CONFIG_KEY_SATURATION).append("=").append(saturation).append("\n");
    s.append(CONFIG_KEY_BRIGHTNESS).append("=").append(brightness).append("\n");
    s.append(CONFIG_KEY_SPEED).append("=").append(speed).append("\n");
    s.append(CONFIG_KEY_ENABLED_CONTENT).append("=").append(enabledContentIndices).append("\n");
    s.append(CONFIG_KEY_FLIP_VERTICAL).append("=").append(flipVertical).append("\n");
    s.append(CONFIG_KEY_CYCLE_DURATION).append("=").append(cycleDuration).append("\n");

    try (var out = new FileWriter(new File(CONFIG_FILE).getAbsoluteFile())) {
      out.write(s.toString());
    } catch (IOException e) {
      Log.error(e);
    }
  }

  private int calcFrameOffset(int[][] modules, int m) {
    for (var i = 0; i < modules.length; i++) {
      for (var j = 0; j < modules[i].length; j++) {
        if (modules[i][j] == m) {
          return (3 * getDimK() * (i * getModuleDimI() + j * getModuleDimJ() * getDimI()));
        }
      }
    }
    throw new IllegalArgumentException("Invalid module number for getFrameOffset()");
  }

  private static String getString(Properties properties, String key, String defaultValue) {
    return properties.getProperty(key, defaultValue);
  }

  private static int getInt(Properties properties, String key, int defaultValue) {
    try {
      return Integer.parseInt(properties.getProperty(key, Integer.toString(defaultValue)));
    } catch (NumberFormatException t) {
      return defaultValue;
    }
  }

  private static float getFloat(Properties properties, String key, float defaultValue) {
    try {
      return Float.parseFloat(properties.getProperty(key, Float.toString(defaultValue)));
    } catch (NumberFormatException t) {
      return defaultValue;
    }
  }

  private static boolean getBoolean(Properties properties, String key, boolean defaultValue) {
    return Boolean.parseBoolean(properties.getProperty(key, Boolean.toString(defaultValue)));
  }

  private static BitSet getBitSet(Properties properties, String key, int numBits, boolean defaultValue) {
    String property = properties.getProperty(key, null);
    if (property == null) {
      BitSet bitSet = new BitSet(numBits);
      bitSet.set(0, numBits, defaultValue);
      return bitSet;
    } else {
      return stringToBitSet(property, numBits);
    }
  }

  public static BitSet stringToBitSet(String s, int numBits) {
    BitSet bitSet = new BitSet(numBits);
    if (!s.isEmpty()) {
      String[] indices = s.split(",");
      for (String index : indices) {
        try {
          int i = Integer.parseInt(index.trim());
          if (i < numBits) bitSet.set(i);
        } catch (NumberFormatException e) {}
      }
    }
    return bitSet;
  }
}
