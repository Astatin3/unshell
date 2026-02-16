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

function startHttpRequest(callback) {
  var xmlHttp = new XMLHttpRequest();
  xmlHttp.onreadystatechange = function () {
    if (xmlHttp.readyState !== 4) return;

    if (xmlHttp.status == 200) callback(true, xmlHttp.responseText);
    else if (xmlHttp.status == 401) callback(false, "Unauthorized");
    else if (xmlHttp.status == 500) callback(false, "Internal Server Error");
    else callback(false, xmlHttp.responseText);
  };
  return xmlHttp;
}

export function httpGet(theUrl, callback) {
  var xmlHttp = startHttpRequest(callback);
  xmlHttp.open("GET", theUrl, true); // true for asynchronous
  xmlHttp.setRequestHeader("Content-Type", "application/json");
  xmlHttp.send(null);
}

export function httpPost(url, body, callback) {
  var xmlHttp = startHttpRequest(callback);
  xmlHttp.open("POST", url, true);
  xmlHttp.setRequestHeader("Content-Type", "application/json");
  // var data = JSON.stringify({ email: "[email protected]", password: "101010" });
  xmlHttp.send(body);
}

export function httpGetAuth(theUrl, auth, callback) {
  var xmlHttp = startHttpRequest(callback);
  xmlHttp.open("GET", theUrl, true); // true for asynchronous
  xmlHttp.setRequestHeader("Content-Type", "application/json");
  xmlHttp.setRequestHeader("authorization", auth);
  xmlHttp.send(null);
}

export function httpPostAuth(url, auth, body, callback) {
  var xmlHttp = startHttpRequest(callback);
  xmlHttp.open("POST", url, true);
  xmlHttp.setRequestHeader("Content-Type", "application/json");
  xmlHttp.setRequestHeader("authorization", auth);
  // var data = JSON.stringify({ email: "[email protected]", password: "101010" });
  xmlHttp.send(body);
}
