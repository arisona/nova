package org.corebounce.nova;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.URLDecoder;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

import org.corebounce.util.Log;
import org.json.JSONArray;

import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;

import java.util.Collections;
import java.util.Set;
import java.util.stream.Collectors;

public final class UIServer {

    private static final Map<String, String> CONTENT_TYPES = Map.of(
            "html", "text/html; charset=UTF-8",
            "css", "text/css; charset=UTF-8",
            "js", "application/javascript; charset=UTF-8",
            "json", "application/json; charset=UTF-8",
            "jpg", "image/jpeg",
            "png", "image/png",
            "gif", "image/gif",
            "svg", "image/svg+xml",
            "woff", "font/woff",
            "woff2", "font/woff2"
    );

    private final State state;

    UIServer(State state) throws IOException {
        this.state = state;
        InetSocketAddress host = new InetSocketAddress(state.getPort());
        HttpServer server = HttpServer.create(host, 0);
        server.createContext("/", this::handleContent);
        server.createContext("/api/", this::handleAPI);
        server.start();
        Log.info("Server is running at http://" + host.getHostName() + ":" + host.getPort() + "/");
    }

    private void handleContent(HttpExchange he) throws IOException {
        URI uri = he.getRequestURI();
        Log.info("Handle content " + uri);
        try {
            String res = "/www" + (uri.getPath().equals("/") ? "/index.html" : uri.getPath());
            String ext = res.contains(".") ? res.substring(res.lastIndexOf(".") + 1) : "";
            String contentType = CONTENT_TYPES.get(ext);
            if (contentType == null) {
                throw new IllegalArgumentException("Invalid contet request " + res + " (" + ext + ")");
            }
            Log.info("Handle local resource " + res + " " + contentType);
            he.getResponseHeaders().set("Content-Type", contentType);
            he.sendResponseHeaders(200, 0);
            try (InputStream is = getClass().getResourceAsStream(res); OutputStream os = he.getResponseBody()) {
                is.transferTo(os);
            }
        } catch (IOException | IllegalArgumentException t) {
            Log.warning(t);
            he.sendResponseHeaders(404, -1);
        }
    }

    private void handleAPI(HttpExchange he) throws IOException {
        URI uri = he.getRequestURI();
        Log.info("Handle " + uri);
        String response = "";
        try {
            String param = uri.getPath().split("[/]")[2];
            String query = uri.getQuery();
            String value = "";
            if (query != null) {
                String[] parts = query.split("[=]");
                if (parts.length == 2) {
                    value = URLDecoder.decode(parts[1], "UTF-8");
                }
            }

            switch (param) {
                case "enabled-content-indices" ->
                    state.setEnabledContentIndices(value);
                case "selected-content-index" ->
                    state.setSelectedContentIndex(Integer.parseInt(value));
                case "hue" ->
                    state.setHue(Float.parseFloat(value));
                case "saturation" ->
                    state.setSaturation(Float.parseFloat(value));
                case "brightness" ->
                    state.setBrightness(Float.parseFloat(value));
                case "flip-vertical" ->
                    state.setFlipVertical(Boolean.parseBoolean(value));
                case "cycle-duration" ->
                    state.setCycleDuration(Float.parseFloat(value));
                case "ethernet-interface" ->
                    state.setEthernetInterface(value);
                case "module0-address" ->
                    state.setModule0Address(value);
                case "speed" ->
                    state.setSpeed(Float.parseFloat(value));
                case "restore" ->
                    state.restore();
                case "reset" ->
                    NOVAControl.get().novaReset();
                case "reload" -> {
                    Log.info("User requested reload. Exiting.");
                    System.exit(0);
                }
                case "get-state" -> {
                    response = getState();
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
        try (OutputStream os = he.getResponseBody()) {
            os.write(response.getBytes());
        }
    }

    private String getState() {
        List<String> availableContent = state.getAvailableContent().stream().map(content -> content.name).toList();
        String indices = state.getEnabledContentIndices();
        Set<Integer> enabledContentIndices
                = indices.isEmpty()
                ? Collections.emptySet()
                : Arrays.stream(indices.split(",")).map(Integer::parseInt).collect(Collectors.toSet());
        String result = String.format("""
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
                    "module0-address": "%s"
                }
                """,
                new JSONArray(availableContent),
                new JSONArray(enabledContentIndices),
                state.getSelectedContentIndex(),
                state.getHue(), state.getSaturation(), state.getBrightness(), state.getSpeed(),
                state.isFlipVertical(), state.getCycleDuration(),
                state.getEthernetInterface(), state.getModule0Address());
        System.out.println(result);
        return result;
    }
}
