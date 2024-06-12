package org.corebounce.nova;

import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;
import java.io.IOException;
import java.net.InetSocketAddress;
import java.net.URLDecoder;
import java.util.Map;
import java.util.stream.Collectors;
import org.json.JSONArray;

public final class UIServer {

  private static final Map<String, String> CONTENT_TYPES = Map.of(
    "html",
    "text/html; charset=UTF-8",
    "css",
    "text/css; charset=UTF-8",
    "js",
    "application/javascript; charset=UTF-8",
    "json",
    "application/json; charset=UTF-8",
    "jpg",
    "image/jpeg",
    "png",
    "image/png",
    "gif",
    "image/gif",
    "svg",
    "image/svg+xml",
    "woff",
    "font/woff",
    "woff2",
    "font/woff2"
  );

  private final State state;

  UIServer(State state) throws IOException {
    this.state = state;
    var host = new InetSocketAddress(state.getPort());
    var server = HttpServer.create(host, 0);
    server.createContext("/", this::handleContent);
    server.createContext("/api/", this::handleAPI);
    server.start();
    Log.info("Server is running at http://" + host.getHostName() + ":" + host.getPort() + "/");
  }

  private void handleContent(HttpExchange he) throws IOException {
    var uri = he.getRequestURI();
    Log.info("Handle content " + uri);
    try {
      var res = "/www" + (uri.getPath().equals("/") ? "/index.html" : uri.getPath());
      var ext = res.contains(".") ? res.substring(res.lastIndexOf(".") + 1) : "";
      var contentType = CONTENT_TYPES.get(ext);
      if (contentType == null) {
        throw new IllegalArgumentException("Invalid contet request " + res + " (" + ext + ")");
      }
      Log.info("Handle local resource " + res + " " + contentType);
      he.getResponseHeaders().set("Content-Type", contentType);
      he.sendResponseHeaders(200, 0);
      try (var is = getClass().getResourceAsStream(res); var os = he.getResponseBody()) {
        is.transferTo(os);
      }
    } catch (IOException | IllegalArgumentException t) {
      Log.warning(t);
      he.sendResponseHeaders(404, -1);
    }
  }

  private void handleAPI(HttpExchange he) throws IOException {
    var uri = he.getRequestURI();
    Log.info("Handle api " + uri);
    var response = "";
    try {
      var param = uri.getPath().split("[/]")[2];
      var query = uri.getQuery();
      var value = "";
      if (query != null) {
        var parts = query.split("[=]");
        if (parts.length == 2) {
          value = URLDecoder.decode(parts[1], "UTF-8");
        }
      }

      switch (param) {
        case "enabled-content-indices" -> state.setEnabledContentIndices(
          State.stringToBitSet(value, state.getAvailableContent().size())
        );
        case "selected-content-index" -> state.setSelectedContentIndex(Integer.parseInt(value));
        case "hue" -> state.setHue(Float.parseFloat(value));
        case "saturation" -> state.setSaturation(Float.parseFloat(value));
        case "brightness" -> state.setBrightness(Float.parseFloat(value));
        case "flip-vertical" -> state.setFlipVertical(Boolean.parseBoolean(value));
        case "cycle-duration" -> state.setCycleDuration(Float.parseFloat(value));
        case "ethernet-interface" -> state.setEthernetInterface(value);
        case "module0-address" -> state.setModule0Address(value);
        case "speed" -> state.setSpeed(Float.parseFloat(value));
        case "restore" -> state.restore();
        case "reset" -> NovaControl.get().novaReset();
        case "reload" -> {
          Log.info("User requested reload: exiting");
          System.exit(0);
        }
        case "get-state" -> {
          response = getState();
          he.getResponseHeaders().set("Content-Type", CONTENT_TYPES.get("json"));
        }
        case "get-status" -> {
          response = getStatus();
          he.getResponseHeaders().set("Content-Type", CONTENT_TYPES.get("json"));
        }
        default -> {
          response = "Invalid parameter " + param;
          throw new IllegalArgumentException(response);
        }
      }
      state.writeSettings();
      he.sendResponseHeaders(200, 0);
    } catch (Throwable t) {
      Log.warning(t);
      he.sendResponseHeaders(404, response.length());
    }
    try (var os = he.getResponseBody()) {
      os.write(response.getBytes());
    }
  }

  private String getState() {
    var availableContent = state.getAvailableContent().stream().map(content -> content.name).toList();
    var enabledContentIndices = state.getEnabledContentIndices().stream().boxed().collect(Collectors.toList());
    var result = String.format(
      """
      {
          "available-content": %s,
          "enabled-content-indices": %s,
          "selected-content-index": %d,
          "hue": %f,
          "saturation": %f,
          "brightness": %f,
          "speed": %f,
          "flip-vertical": %b,
          "cycle-duration": %f,
          "ethernet-interface": "%s",
          "module0-address": "%d"
      }
      """,
      new JSONArray(availableContent),
      new JSONArray(enabledContentIndices),
      state.getSelectedContentIndex(),
      state.getHue(),
      state.getSaturation(),
      state.getBrightness(),
      state.getSpeed(),
      state.isFlipVertical(),
      state.getCycleDuration(),
      state.getEthernetInterface(),
      state.getModule0Address()
    );
    //Log.info(result);
    return result;
  }

  private String getStatus() {
    var result = String.format(
      """
      {
          "status-ok": %b,
          "status-message": "%s"
      }
      """,
      state.isStatusOk(),
      state.getStatusMessage()
    );
    return result;
  }
}
