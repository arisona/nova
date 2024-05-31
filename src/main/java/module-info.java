module org.corebounce.nova {
	requires java.logging;
	requires jdk.httpserver;
	requires org.json;

	uses org.jnetpcap.spi.PcapMessagesProvider;
}