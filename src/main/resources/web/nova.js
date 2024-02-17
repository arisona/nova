function httpGet(url) {
    var xmlHttp = new XMLHttpRequest();
    xmlHttp.open('GET', url, true);
    xmlHttp.setRequestHeader('If-Modified-Since', 'Sat, 1 Jan 2005 00:00:00 GMT');
    xmlHttp.send(null);
}
