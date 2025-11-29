var cacheName = "egui-template-pwa";
var filesToCache = [
  "./",
  "./index.html",
  "./eframe_template.js",
  "./eframe_template_bg.wasm",
];

/* Start the service worker and cache all of the app's content */
self.addEventListener("install", function (e) {
  e.waitUntil(
    caches.open(cacheName).then(function (cache) {
      return cache.addAll(filesToCache);
    }),
  );
});

/* Serve cached content when offline */
self.addEventListener("fetch", function (e) {
  e.respondWith(
    caches.match(e.request).then(function (response) {
      return response || fetch(e.request);
    }),
  );
});

// export function httpGet(theUrl) {
//   var xmlHttp = new XMLHttpRequest();
//   xmlHttp.open("GET", theUrl, false); // false for synchronous request
//   xmlHttp.send(null);
//   return xmlHttp.responseText;
// }

export function httpGet(theUrl, callback) {
  var xmlHttp = new XMLHttpRequest();
  xmlHttp.onreadystatechange = function () {
    if (xmlHttp.readyState !== 4) return;

    if (xmlHttp.status == 200) callback(xmlHttp.responseText);
    else alert("Error " + xmlHttp.status + ", " + xmlHttp.responseText);
  };
  xmlHttp.open("GET", theUrl, true); // true for asynchronous
  xmlHttp.setRequestHeader("Content-Type", "application/json");
  xmlHttp.send(null);
}

export function httpPost(url, body, callback) {
  var xmlHttp = new XMLHttpRequest();
  xmlHttp.onreadystatechange = function () {
    if (xmlHttp.readyState !== 4) return;

    if (xmlHttp.status === 200) {
      // var json = JSON.parse(xhr.responseText);
      callback(xmlHttp.responseText);
    } else {
      alert("Error " + xmlHttp.status + ", " + xmlHttp.responseText);
    }
  };

  xmlHttp.open("POST", url, true);
  xmlHttp.setRequestHeader("Content-Type", "application/json");
  // var data = JSON.stringify({ email: "[email protected]", password: "101010" });
  xmlHttp.send(body);
}
