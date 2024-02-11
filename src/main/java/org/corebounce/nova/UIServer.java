package org.corebounce.nova;

import java.io.IOException;
import java.net.InetSocketAddress;
import java.net.URI;

import org.corebounce.util.Log;

import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;

public class UIServer {
    UIServer(int port) throws IOException {
        InetSocketAddress host = new InetSocketAddress(port);
        HttpServer server = HttpServer.create(host, 0);
        server.createContext("/", this::handleContent);
        server.createContext("/nova/", this::handleControl);
        server.start();
        System.out.println("Server is running at http://" + host.getHostName() + ":" + host.getPort() + "/");
    }

    private void handleContent(HttpExchange e) throws IOException {
        URI uri = e.getRequestURI();
        String response = "";
        try {
            if (uri.getPath().equals("/index.html") || uri.getPath().equals("/")) {
                response = getPage();
                e.getResponseHeaders().set("Content-Type", "text/html; charset=UTF-8");
                e.sendResponseHeaders(200, response.length());
            } else {
                response = "Invalid path " + uri.getPath();
                throw new Exception(response);
            }
        } catch (Throwable t) {
            e.sendResponseHeaders(404, response.length());
        }
        try (java.io.OutputStream os = e.getResponseBody()) {
            os.write(response.getBytes());
        }
    }

    private void handleControl(HttpExchange e) throws IOException {
        URI uri = e.getRequestURI();
        String response = "";
        try {
            String param = uri.getPath().split("[/]")[2];
            String query = uri.getQuery();
            String value = query == null ? null : query.split("[=]")[1];

            NOVAControl control = NOVAControl.get();
            switch (param) {
            case "red":
                control.setRed(Double.parseDouble(value));
                break;
            case "green":
                control.setGreen(Double.parseDouble(value));
                break;
            case "blue":
                control.setBlue(Double.parseDouble(value));
                break;
            case "brightness":
                control.setBrightness(Double.parseDouble(value));
                break;
            case "color":
                control.setRed(Integer.parseInt(value.substring(0, 2), 16) << 2);
                control.setGreen(Integer.parseInt(value.substring(2, 4), 16) << 2);
                control.setBlue(Integer.parseInt(value.substring(4, 6), 16) << 2);
                break;
            case "speed":
                control.setSpeed(Double.parseDouble(value));
                break;
            case "content":
                control.setContent((int) Double.parseDouble(value));
                break;
            case "reset":
                control.novaReset();
                break;
            case "reload":
                Log.info("User requested reload. Exiting.");
                System.exit(0);
                break;
            default:
                response = "Invalid parameter " + param;
                throw new Exception(response);
            }
            e.sendResponseHeaders(200, 0);
        } catch (Throwable t) {
            e.sendResponseHeaders(404, response.length());
        }
        try (java.io.OutputStream os = e.getResponseBody()) {
            os.write(response.getBytes());
        }
    }

    private String getPage() {
        NOVAControl control = NOVAControl.get();
        StringBuilder s = new StringBuilder();
        // header, script, style
        s.append("""
                <!DOCTYPE html>
                <html lang='en'>
                  <head>
                    <meta charset='utf-8'>
                    <meta name='viewport' content='width=device-width, initial-scale=1.0'/>
                    <script language='javascript' type='text/javascript'>
                      function httpGet(theUrl) {
                        var xmlHttp = new XMLHttpRequest();
                        xmlHttp.open('GET', theUrl, true);
                        xmlHttp.setRequestHeader('If-Modified-Since', 'Sat, 1 Jan 2005 00:00:00 GMT');
                        xmlHttp.send(null);
                      }
                    </script>
                    <style type='text/css'>
                      body {
                        font-family:sans-serif;
                        background-color:#202020;
                        color:#a0a0a0;
                      }
                      .combo {
                        width:300px;
                        size:4;
                      }
                      .slider {
                        width:220px;
                      }
                      .button {
                      }
                    </style>
                    <title>NOVA</title>
                  </head>
                  <body>
                """);

        // body
        addSelection(s);
        addSlider(s, "Red", "0.0", "1.0", String.valueOf(control.getRed()), "red");
        addSlider(s, "Green", "0.0", "1.0", String.valueOf(control.getGreen()), "green");
        addSlider(s, "Blue", "0.0", "1.0", String.valueOf(control.getBlue()), "blue");
        addSlider(s, "B'ness", "0.1", "1.0", String.valueOf(control.getBrightness()), "brightness");
        addSlider(s, "Speed", "-3.0", "5.0", String.valueOf(control.getSpeed()), "speed");
        addButton(s, "Reset", "reset");
        addButton(s, "Reload", "reload");

        // footer
        s.append("""
                  </body>
                </html>
                """);
        return s.toString();
    }

    private void addSelection(StringBuilder s) {
        NOVAControl control = NOVAControl.get();
        s.append("  <select class='combo' size='4' onChange='httpGet(\"/nova/content?value=\" + this.value);'>\n");
        for (int i = 0; i < control.getContents().size(); ++i) {
            String label = control.getContents().get(i).name;
            if (control.getContent() == i) {
                s.append(String.format("    <option value='%d' selected='selected'>%s</option>\n", i, label));                
            } else {
                s.append(String.format("    <option value='%d'>%s</option>\n", i, label));
            }
        }
        s.append("  </select>");
    }

    private void addSlider(StringBuilder s, String label, String min, String max, String value, String request) {
        s.append(String.format("""
                  <div>
                  <input class='slider' type='range' min='%s' max='%s' value='%s' step='0.01'
                    onInput='httpGet(\"/nova/%s?value=\" + this.value);'/>
                  %s
                  </div>
                """,
                min, max, value, request, label));
    }

    private void addButton(StringBuilder s, String label, String request) {
        s.append(String.format("  <button class='button' type='button' onClick='httpGet(\"/nova/%s\");'>%s</button>\n", request, label));
    }
}
