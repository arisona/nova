(function(){const n=document.createElement("link").relList;if(n&&n.supports&&n.supports("modulepreload"))return;for(const t of document.querySelectorAll('link[rel="modulepreload"]'))c(t);new MutationObserver(t=>{for(const r of t)if(r.type==="childList")for(const i of r.addedNodes)i.tagName==="LINK"&&i.rel==="modulepreload"&&c(i)}).observe(document,{childList:!0,subtree:!0});function o(t){const r={};return t.integrity&&(r.integrity=t.integrity),t.referrerPolicy&&(r.referrerPolicy=t.referrerPolicy),t.crossOrigin==="use-credentials"?r.credentials="include":t.crossOrigin==="anonymous"?r.credentials="omit":r.credentials="same-origin",r}function c(t){if(t.ep)return;t.ep=!0;const r=o(t);fetch(t.href,r)}})();const s="nova-contents",a=["nova-red","nova-green","nova-blue","nova-brightness","nova-speed"],d="nova-reset",l="nova-reload",u="nova-content";f();g();function f(){fetch("/api/get-state").then(e=>{if(!e.ok)throw new Error(e.status.toString());return e}).then(e=>e.json()).then(e=>{a.forEach(o=>{const c=document.getElementById(o);c.value=e[o]});const n=document.getElementById(s);for(let o=0;o<e[s].length;++o)n.appendChild(new Option(e[s][o],o.toString(),!1,o==e[u]))}).catch(function(e){console.error("Request failed ",e)})}function g(){document.getElementById(s).addEventListener("change",e=>{const n=e.target;fetch(`/api/${n.id}?value=${n.value}`)}),a.forEach(e=>document.getElementById(e).addEventListener("input",n=>{const o=n.target;fetch(`/api/${o.id}?value=${o.value}`)})),document.getElementById(d).addEventListener("click",e=>{const n=e.target;fetch(`/api/${n.id}`)}),document.getElementById(l).addEventListener("click",e=>{const n=e.target;fetch(`/api/${n.id}`)})}
