module org.corebounce.nova {
  requires java.logging;
  requires jdk.httpserver;
  requires org.json;

  // add this when switching back to Maven jnetpcap, and remove 'uses...' below
  //requires org.jnetpcap;

  uses org.jnetpcap.spi.PcapMessagesProvider;
}
