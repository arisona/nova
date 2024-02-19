package org.corebounce.nova;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.URI;
import java.util.List;
import java.util.Map;

import org.corebounce.util.Log;
import org.json.JSONArray;

import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;

public class UIServer {
    private static final Map<String,String> CONTENT_TYPES = Map.of(
        "html", "text/html; charset=UTF-8",
        "css",  "text/css; charset=UTF-8",
        "js",   "application/javascript; charset=UTF-8",
        "json", "application/json; charset=UTF-8",
        "jpg",  "image/jpeg",
        "png",  "image/png",
        "gif",  "image/gif"
    );

    UIServer(int port) throws IOException {
        InetSocketAddress host = new InetSocketAddress(port);
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
            if (contentType == null)
                throw new IllegalArgumentException("Invalid contet request " + res + " (" + ext + ")");
            Log.info("Handle local resource " + res + " " + contentType);
            he.getResponseHeaders().set("Content-Type", contentType);
            he.sendResponseHeaders(200, 0);
            try (InputStream is = getClass().getResourceAsStream(res); OutputStream os = he.getResponseBody()) {
                is.transferTo(os);
            }
        } catch (Throwable t) {
            Log.warning(t);
            he.sendResponseHeaders(404, -1);
        }
    }

    private void handleAPI(HttpExchange he) throws IOException {
        URI uri = he.getRequestURI();
        Log.info("Handle control " + uri);
        String response = "";
        try {
            String param = uri.getPath().split("[/]")[2];
            String query = uri.getQuery();
            String value = query == null ? null : query.split("[=]")[1];

            NOVAControl control = NOVAControl.get();
            switch (param) {
                case "nova-red":
                    control.setRed(Double.parseDouble(value));
                    break;
                case "nova-green":
                    control.setGreen(Double.parseDouble(value));
                    break;
                case "nova-blue":
                    control.setBlue(Double.parseDouble(value));
                    break;
                case "nova-brightness":
                    control.setBrightness(Double.parseDouble(value));
                    break;
                case "nova-speed":
                    control.setSpeed(Double.parseDouble(value));
                    break;
                case "nova-contents":
                    control.setContent(Integer.parseInt(value));
                    break;
                case "nova-reset":
                    control.novaReset();
                    break;
                case "nova-reload":
                    Log.info("User requested reload. Exiting.");
                    System.exit(0);
                    break;
                case "get-state":
                    response = getState();
                    he.getResponseHeaders().set("Content-Type", CONTENT_TYPES.get("json"));
                    break;
                default:
                    response = "Invalid parameter " + param;
                    throw new IllegalArgumentException(response);
            }
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
        NOVAControl c = NOVAControl.get();
        List<String> contents = c.getContents().stream().map(content -> content.name).toList();
        return String.format("""
                {
                    "nova-red": %f,
                    "nova-green": %f,
                    "nova-blue": %f,
                    "nova-brightness": %f,
                    "nova-speed": %f,
                    "nova-content": %d,
                    "nova-contents": %s
                }
                """,
                c.getRed(), c.getGreen(), c.getBlue(), c.getBrightness(), c.getSpeed(),
                c.getContent(), new JSONArray(contents));
    }
}
