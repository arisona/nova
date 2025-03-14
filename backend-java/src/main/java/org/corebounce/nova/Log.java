package org.corebounce.nova;

import java.io.PrintWriter;
import java.io.StringWriter;
import java.lang.reflect.InvocationTargetException;

public final class Log {

  private static String getStackTrace(final Throwable throwable) {
    final StringWriter sw = new StringWriter();
    final PrintWriter pw = new PrintWriter(sw, true);
    throwable.printStackTrace(pw);
    return sw.getBuffer().toString();
  }

  private static String getMsg(Throwable t) {
    String result = t instanceof InvocationTargetException
      ? ((InvocationTargetException) t).getTargetException().getMessage()
      : t.getMessage();
    return result + " -- " + getStackTrace(t);
  }

  private static void log(String type, String msg, Throwable t) {
    System.out.print("[" + type + "]");
    if (msg != null) {
      System.out.print(" " + msg);
    }
    if (t != null) {
      System.out.print("\n" + getMsg(t));
    }
    System.out.println();
  }

  public static void info(String msg) {
    log("info", msg, null);
  }

  public static void info(Throwable t) {
    log("info", null, t);
  }

  public static void info(String msg, Throwable t) {
    log("info ", msg, t);
  }

  public static void warning(String msg) {
    log("warn", msg, null);
  }

  public static void warning(Throwable t) {
    log("warn", null, t);
  }

  public static void warning(String msg, Throwable t) {
    log("warn", msg, t);
  }

  public static void error(String msg) {
    log("error", msg, null);
  }

  public static void error(Throwable t) {
    log("error", null, t);
  }

  public static void error(String msg, Throwable t) {
    log("error", msg, t);
  }

  public static void fatal(String msg) {
    log("fatal", msg, null);
    System.exit(-1);
  }

  public static void fatal(Throwable t) {
    log("fatal", null, t);
    System.exit(-1);
  }

  public static void fatal(String msg, Throwable t) {
    log("fatal", msg, t);
    System.exit(-1);
  }
}
