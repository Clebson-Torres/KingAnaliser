import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

/* ══════════════════════════════════════════════
   KingNetworkTools — Main Application
   ══════════════════════════════════════════════ */

// ─── DOM refs ───
const D = (id) => document.getElementById(id);
const sidebarNav = D("sidebar-nav");
const viewContainer = D("view-container");
const toastContainer = D("toast-container");
const hostInput = D("host-input");
const trHost = D("tr-host-input");
const portsHost = D("ports-host");
const dnsInput = D("dns-input");
const subnetInput = D("subnet-input");
const historySection = D("history-section");
const historyList = D("history-list");
const graphSection = D("graph-section");
const latencyCanvas = D("latency-canvas");
const graphStats = D("graph-stats");
const gatewayWarning = D("gateway-warning");
const scanTimeout = D("scan-timeout");
const scanTimeoutLabel = D("scan-timeout-label");
const scanProgress = D("scan-progress");
const scanProgressFill = D("scan-progress-fill");
const scanProgressText = D("scan-progress-text");

// ─── State ───
let portList = [];
let continuousPingRunning = false;
let continuousPingTimer = null;
let pingData = [];
const MAX_PING_POINTS = 60;
let isRunning = {};
const HISTORY_KEY = "kinganaliser_history";

// ─── Toast ───
function toast(msg, type = "info") {
  const el = document.createElement("div");
  el.className = "toast " + type;
  el.textContent = msg;
  toastContainer.appendChild(el);
  setTimeout(() => { el.style.opacity = "0"; setTimeout(() => el.remove(), 300); }, 3500);
}

// ─── Navigation ───
document.querySelectorAll(".nav-item[data-view]").forEach(btn => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".nav-item").forEach(b => b.classList.remove("active"));
    btn.classList.add("active");
    const view = btn.dataset.view;
    document.querySelectorAll(".view").forEach(v => v.classList.remove("active"));
    const target = D("view-" + view);
    if (target) target.classList.add("active");
    D("view-title").textContent = btn.querySelector("span:last-child").textContent;
  });
});

// ─── Utility ───
function showOutput(containerId) {
  const el = D(containerId);
  if (el) el.classList.remove("hidden");
}

function appendOutput(containerId, text) {
  const el = D(containerId);
  if (!el) return;
  el.innerHTML += text + "\n";
  el.scrollTop = el.scrollHeight;
}

function clearOutput(containerId) {
  const el = D(containerId);
  if (el) el.innerHTML = "";
}

function toTable(headers, rows) {
  const colWidths = headers.map((h, i) => Math.max(h.length, ...rows.map(r => String(r[i] || "").length)));
  let out = "";
  const line = headers.map((h, i) => "\u2500".repeat(colWidths[i] + 2)).join("\u2500");
  out += "  " + line + "\n";
  out += "  " + headers.map((h, i) => " " + h.padEnd(colWidths[i]) + " ").join("|") + "\n";
  out += "  " + line + "\n";
  for (const row of rows) {
    out += "  " + row.map((v, i) => " " + String(v).padEnd(colWidths[i]) + " ").join("|") + "\n";
  }
  out += "  " + line + "\n";
  return out;
}

function plainTable(headers, rows) {
  const safeRows = rows.length ? rows : [["-".repeat(headers.length)]];
  const colWidths = headers.map((h, i) => Math.max(String(h).length, ...safeRows.map(r => String(r[i] ?? "").length)));
  const sep = "+-" + colWidths.map(w => "-".repeat(w)).join("-+-") + "-+";
  const formatRow = (row) => "| " + headers.map((_, i) => String(row[i] ?? "").padEnd(colWidths[i])).join(" | ") + " |";
  return [
    sep,
    formatRow(headers),
    sep,
    ...rows.map(formatRow),
    sep,
  ].join("\n");
}

function pct(v) {
  return typeof v === "number" ? v.toFixed(1) + "%" : "-";
}

function qualityClass(val, good, warn) {
  if (val <= good) return "quality-good";
  if (val <= warn) return "quality-warn";
  return "quality-bad";
}

function fmtMs(ms) {
  return (ms !== undefined && ms !== null) ? (typeof ms === "number" ? ms.toFixed(1) + " ms" : ms + " ms") : "-";
}

function badge(quality, color) {
  const cls = color === "green" ? "badge-green" : color === "yellow" ? "badge-yellow" : "badge-red";
  return '<span class="badge ' + cls + '">' + quality + "</span>";
}

function setRunning(key, running) {
  isRunning[key] = running;
}

function isBusy(key) { return !!isRunning[key]; }

function setStatusCard(id, state, text) {
  const el = D(id);
  if (!el) return;
  el.classList.remove("good", "warn", "bad", "unknown");
  el.classList.add(state);
  const value = el.querySelector(".status-card-value");
  if (value) value.textContent = text;
}

async function withLoading(msg, key, outId, fn) {
  if (isBusy(key)) return;
  setRunning(key, true);
  try {
    return await fn();
  } catch (err) {
    appendOutput(outId, '<span class="error">[ERRO] ' + err + "</span>");
    toast(err, "error");
  } finally {
    setRunning(key, false);
  }
}

// ─── Dashboard ───
async function loadDashboard() {
  try {
    setStatusCard("status-gateway", "unknown", "Verificando...");
    setStatusCard("status-dns", "unknown", "Verificando...");
    setStatusCard("status-internet", "unknown", "Verificando...");
    setStatusCard("status-stability", "unknown", "Verificando...");

    const [ifaces, ipPub] = await Promise.all([
      invoke("get_network_interfaces"),
      invoke("get_public_ip").catch(() => "---"),
    ]);

    const active = ifaces.find(i => i.is_up) || ifaces[0];
    if (active) {
      D("card-local-ip").querySelector(".card-value").textContent = active.ip;
      D("card-local-ip").querySelector(".card-sub").textContent = active.name;
    }

    D("card-public-ip").querySelector(".card-value").textContent = ipPub;
    setStatusCard("status-internet", ipPub === "---" ? "bad" : "good", ipPub === "---" ? "Sem IP público" : "Online");

    try {
      const info = await invoke("get_gateway_info");
      const gw = info.gateways?.[0];
      if (gw) {
        D("card-gateway").querySelector(".card-value").textContent = gw.ip;
        const lat = gw.latency_ms !== null ? gw.latency_ms.toFixed(1) + "ms" : "---";
        D("card-gateway").querySelector(".card-sub").textContent = lat + (info.has_multiple ? " (duplo!)" : "");
        if (info.has_multiple) {
          gatewayWarning.textContent = "⚠ " + info.warning;
          gatewayWarning.classList.remove("hidden");
        }
        const state = gw.reachable ? (info.has_multiple ? "warn" : "good") : "bad";
        const label = gw.reachable ? lat + (info.has_multiple ? " / múltiplo" : "") : "Inalcançável";
        setStatusCard("status-gateway", state, label);
      }
    } catch {
      setStatusCard("status-gateway", "bad", "Não detectado");
    }

    try {
      const p = await invoke("ping", { host: "8.8.8.8", count: 3 });
      D("card-latency").querySelector(".card-value").textContent = p.avg_ms.toFixed(1) + "ms";
      D("card-latency").querySelector(".card-sub").textContent = p.quality + " | perda " + p.loss_pct.toFixed(0) + "%";
      const dot = D("status-indicator");
      dot.className = "status-dot " + (p.quality_color === "red" ? "red" : p.quality_color === "yellow" ? "yellow" : "green");
      setStatusCard("status-stability", p.quality_color === "red" ? "bad" : p.quality_color === "yellow" ? "warn" : "good", p.quality + " / perda " + p.loss_pct.toFixed(0) + "%");
    } catch {
      setStatusCard("status-stability", "bad", "Ping falhou");
    }

    const os = navigator.platform || "desconhecido";
    D("card-uptime").querySelector(".card-value").textContent = "Online";
    D("card-uptime").querySelector(".card-sub").textContent = os;

    try {
      const dns = await invoke("dns_lookup", { host: "google.com" });
      D("card-dns").querySelector(".card-value").textContent = dns.addresses[0] || "---";
      setStatusCard("status-dns", dns.addresses?.length ? "good" : "bad", dns.addresses?.length ? "Resolvido" : "Sem resposta");
    } catch {
      setStatusCard("status-dns", "bad", "Falha DNS");
    }

  } catch (e) {
    console.error("Dashboard load error:", e);
  }
}

// ─── Ping ───
async function showPing() {
  const host = hostInput.value.trim() || "8.8.8.8";
  await withLoading("Ping...", "ping", "ping-output", async () => {
    clearOutput("ping-output");
    const result = await invoke("ping", { host, count: 10 });
    let out = "  Host: " + result.host + "\n";
    out += toTable(
      ["Enviados", "Recebidos", "Perdidos", "Perda%"],
      [[String(result.packets_sent), String(result.packets_received), String(result.packets_sent - result.packets_received),
        '<span class="' + (result.loss_pct === 0 ? "quality-good" : result.loss_pct <= 2 ? "quality-warn" : "quality-bad") + '">' + result.loss_pct.toFixed(1) + "%</span>"]]
    );
    if (result.packets_received > 0) {
      out += toTable(
        ["Mínimo", "Média", "Máximo", "Jitter"],
        [[fmtMs(result.min_ms), '<span class="' + qualityClass(result.avg_ms, 30, 80) + '">' + result.avg_ms.toFixed(1) + " ms</span>",
          fmtMs(result.max_ms), '<span class="' + (result.jitter_ms < 5 ? "quality-good" : result.jitter_ms < 20 ? "quality-warn" : "quality-bad") + '">' + result.jitter_ms.toFixed(1) + " ms</span>"]]
      );
    }
    out += '  Avaliação: ' + badge(result.quality, result.quality_color);
    appendOutput("ping-output", out);
    toast("Ping concluído: " + result.quality, "success");
  });
}

// ─── Traceroute ───
async function showTraceroute() {
  const host = trHost.value.trim() || "8.8.8.8";
  await withLoading("Traceroute...", "traceroute", "traceroute-output", async () => {
    clearOutput("traceroute-output");
    const hops = await invoke("trace_route", { host });
    const rows = hops.map(h => {
      const sCls = h.status === "ok" ? "quality-good" : h.status === "warning" ? "quality-warn" : "quality-bad";
      return [
        String(h.hop_number),
        h.address,
        h.hostname || "-",
        h.min_ms > 0 ? h.min_ms.toFixed(1) + "ms" : "-",
        h.avg_ms > 0 ? '<span class="' + qualityClass(h.avg_ms, 30, 80) + '">' + h.avg_ms.toFixed(1) + "ms</span>" : "-",
        h.max_ms > 0 ? h.max_ms.toFixed(1) + "ms" : "-",
        '<span class="' + (h.loss_pct === 0 ? "quality-good" : h.loss_pct <= 5 ? "quality-warn" : "quality-bad") + '">' + h.loss_pct.toFixed(0) + "%</span>",
        '<span class="' + sCls + '">' + h.status + "</span>",
      ];
    });
    appendOutput("traceroute-output", toTable(["#", "IP", "Hostname", "Min", "Avg", "Max", "Perda%", "Status"], rows));
    const crit = hops.filter(h => h.status === "critical");
    if (crit.length) appendOutput("traceroute-output", '\n<span class="quality-bad">⚠ ' + crit.length + " hop(s) crítico(s)</span>");
    const replies = hops.filter(h => h.address !== "*" && h.status !== "no_reply");
    if (hops.length <= 1 || replies.length <= 1) {
      appendOutput("traceroute-output", '\n<span class="quality-warn">Rota parcialmente filtrada. O app tentou traceroute padrão, TCP/443, ICMP e tracepath; firewalls no caminho podem ocultar os hops intermediários.</span>');
    }
  });
}

// ─── MTR ───
async function showMtr() {
  const host = trHost.value.trim() || "8.8.8.8";
  await withLoading("MTR...", "mtr", "traceroute-output", async () => {
    clearOutput("traceroute-output");
    appendOutput("traceroute-output", "  Executando MTR para " + host + " com 5 ciclos...\n");
    const hops = await invoke("run_mtr", { host, cycles: 5 });
    const rows = hops.map(h => [
      String(h.hop), h.host,
      '<span class="' + (h.loss_pct === 0 ? "quality-good" : "quality-bad") + '">' + h.loss_pct.toFixed(1) + "%</span>",
      '<span class="' + qualityClass(h.avg_ms, 30, 80) + '">' + h.avg_ms.toFixed(1) + "ms</span>",
      h.best_ms.toFixed(1) + "ms", h.worst_ms.toFixed(1) + "ms", h.jitter_ms.toFixed(1) + "ms",
      '<span class="' + (h.quality === "ok" ? "quality-good" : h.quality === "warning" ? "quality-warn" : "quality-bad") + '">' + h.quality + "</span>",
    ]);
    appendOutput("traceroute-output", toTable(["Hop", "Host", "Perda", "Média", "Melhor", "Pior", "Jitter", "Qualid."], rows));
    const replies = hops.filter(h => h.host !== "*" && h.quality !== "no_reply" && h.loss_pct < 100);
    if (hops.length <= 1 || replies.length <= 1) {
      appendOutput("traceroute-output", '\n<span class="quality-warn">MTR recebeu poucos hops. Se o binário mtr não tiver permissão/socket disponível, o app usa traceroute como fallback.</span>');
    }
  });
}

// ─── IP ───
async function showLocalIp() {
  await withLoading("IP Local...", "local-ip", "ip-output", async () => {
    clearOutput("ip-output");
    const ifaces = await invoke("get_network_interfaces");
    const rows = ifaces.map(i => ['<span class="' + (i.is_up ? "quality-good" : "quality-bad") + '">' + i.name + "</span>", i.ip, i.mac || "-", i.is_up ? "Ativa" : "Inativa"]);
    appendOutput("ip-output", toTable(["Interface", "IP", "MAC", "Status"], rows));
  });
}

async function showPublicIp() {
  await withLoading("IP Público...", "public-ip", "ip-output", async () => {
    clearOutput("ip-output");
    const ip = await invoke("get_public_ip");
    appendOutput("ip-output", "  IP Público: " + ip);
  });
}

async function showPublicIpInfo() {
  await withLoading("Geo IP...", "geo-ip", "ip-output", async () => {
    clearOutput("ip-output");
    const info = await invoke("get_public_ip_info");
    appendOutput("ip-output",
      "  IPv4:      " + info.ipv4 + "\n" +
      "  IPv6:      " + (info.ipv6 || "N/A") + "\n" +
      "  País:      " + info.country + " (" + info.country_code + ")\n" +
      "  Cidade:    " + info.city + "\n" +
      "  ISP:       " + info.isp + "\n" +
      "  ASN:       " + info.asn + " — " + info.as_name + "\n" +
      "  Proxy:     " + (info.is_proxy ? "Sim" : "Não") + "\n" +
      "  Datacenter:" + (info.is_hosting ? "Sim" : "Não")
    );
  });
}

async function showGateway() {
  await withLoading("Gateway...", "gateway", "ip-output", async () => {
    clearOutput("ip-output");
    const info = await invoke("get_gateway_info");
    gatewayWarning.classList.add("hidden");
    if (info.has_multiple) {
      gatewayWarning.textContent = "⚠ " + (info.warning || "Gateway duplo detectado");
      gatewayWarning.classList.remove("hidden");
    }
    const rows = info.gateways.map(g => [
      g.ip, g.interface, String(g.metric),
      g.latency_ms !== null ? '<span class="' + qualityClass(g.latency_ms, 30, 80) + '">' + g.latency_ms.toFixed(1) + " ms</span>" : "N/A",
      '<span class="' + (g.reachable ? "quality-good" : "quality-bad") + '">' + (g.reachable ? "Sim" : "Não") + "</span>",
      g.is_primary ? '<span class="quality-good">★</span>' : "",
    ]);
    appendOutput("ip-output", toTable(["IP", "Interface", "Métrica", "Latência", "Alcançável", "Primário"], rows));
  });
}

// ─── DNS ───
async function showDnsLookup() {
  const host = dnsInput.value.trim();
  if (!host) return;
  await withLoading("DNS...", "dns", "dns-output", async () => {
    clearOutput("dns-output");
    const result = await invoke("dns_lookup", { host });
    appendOutput("dns-output", "  " + result.host + "\n" + result.addresses.map(ip => "    - " + ip).join("\n") + "\n  Reverso: " + (result.reverse || "N/A"));
  });
}

async function showDnsBench() {
  await withLoading("DNS Benchmark...", "dns-bench", "dns-output", async () => {
    clearOutput("dns-output");
    appendOutput("dns-output", "  Testando servidores DNS...\n");
    const results = await invoke("benchmark_dns");
    const rows = results.map(r => [
      '<span class="server-name">' + r.name + "</span>" + (r.best ? ' ★' : ""),
      r.ip,
      '<span class="' + qualityClass(r.latency_ms, 30, 80) + '">' + r.latency_ms + "ms</span>",
      r.status === "OK" ? '<span class="quality-good">OK</span>' : '<span class="quality-bad">' + r.status + "</span>",
    ]);
    appendOutput("dns-output", toTable(["Servidor", "IP", "Latência", "Status"], rows));
  });
}

// ─── Ports ───
async function showListeningPorts() {
  await withLoading("Portas...", "listening", "ports-output", async () => {
    clearOutput("ports-output");
    const ports = await invoke("get_listening_ports");
    const rows = ports.map(p => [String(p.port), p.protocol, p.state]);
    appendOutput("ports-output", toTable(["Porta", "Protocolo", "Estado"], rows) + "\n  Total: " + ports.length);
  });
}

async function showPortScan() {
  const host = portsHost.value.trim() || "127.0.0.1";
  const timeout = parseInt(scanTimeout.value, 10);
  await withLoading("Scan TCP...", "scan", "ports-output", async () => {
    clearOutput("ports-output");
    appendOutput("ports-output", "  Timeout: " + timeout + "ms\n");
    const results = await invoke("scan_ports", { host, portsList: portList, timeout_ms: timeout });
    const open = results.filter(r => r.state === "open");
    const filtered = results.filter(r => r.state === "filtered");
    const closed = results.filter(r => r.state === "closed");
    if (open.length) {
      appendOutput("ports-output", "\n  ABERTAS (" + open.length + "):");
      appendOutput("ports-output", toTable(["Porta", "Serviço", "Estado", "Resposta"],
        open.map(r => [String(r.port), r.service, '<span class="quality-good">aberta</span>', r.response_ms ? r.response_ms.toFixed(0) + "ms" : "-"])
      ));
    }
    if (filtered.length) appendOutput("ports-output", "\n  FILTRADAS: " + filtered.map(r => r.port).join(", "));
    if (closed.length) appendOutput("ports-output", "\n  FECHADAS: " + closed.map(r => r.port).join(", "));
    appendOutput("ports-output", "\n  Total: " + results.length + " | Abertas: " + open.length + " | Filtradas: " + filtered.length);
  });
}

// ─── Network Scan ───
async function showNetworkScan() {
  await withLoading("Scan Rede...", "scan-network", "scan-output", async () => {
    clearOutput("scan-output");
    const subnet = subnetInput.value.trim() || null;
    scanProgress.classList.remove("hidden");
    scanProgressFill.style.width = "0%";
    scanProgressText.textContent = "Escaneando...";
    appendOutput("scan-output", "  Sub-rede: " + (subnet || "auto") + "\n");
    const result = await invoke("scan_network", { subnet });
    scanProgress.classList.add("hidden");
    appendOutput("scan-output", "  Hosts encontrados: " + result.hosts_up + " de " + result.total_hosts + " (" + result.scan_duration_secs.toFixed(1) + "s)\n");
    if (result.hosts.length) {
      appendOutput("scan-output", toTable(["IP", "Hostname", "MAC", "Fabricante", "Latência", "Portas"],
        result.hosts.map(h => [
          h.ip + (h.is_gateway ? ' <span class="quality-good">[GW]</span>' : ""),
          h.hostname || "-", h.mac || "-", h.vendor || "-",
          h.latency_ms !== null ? h.latency_ms.toFixed(1) + "ms" : "-",
          h.open_ports.length ? h.open_ports.join(", ") : "-",
        ])
      ));
    }
    toast("Scan concluído: " + result.hosts_up + " hosts encontrados", "success");
  });
}

// ─── HTTP ───
async function showHttpTiming() {
  await withLoading("HTTP...", "http", "http-output", async () => {
    clearOutput("http-output");
    const targets = await invoke("get_http_targets");
    const results = await invoke("test_http_timing", { urls: targets });
    for (const t of results) {
      appendOutput("http-output", "  " + t.url + ": " + t.total_ms.toFixed(0) + "ms (" + t.quality + ")");
    }
  });
}

// ─── Report ───
async function showFullReport() {
  await withLoading("Relatório...", "report", "report-output", async () => {
    clearOutput("report-output");
    const host = hostInput.value.trim() || "8.8.8.8";
    appendOutput("report-output", "  Coletando dados...\n");

    let ipLocalText="", ipPubText="", dnsText="", pingText="",
        tracerouteText="", portsText="", scanText="",
        gatewayText="", dnsBenchText="", httpText="", ifaceStatsText="";

    try {
      const ifaces = await invoke("get_network_interfaces");
      ipLocalText = plainTable(["Interface", "IP", "MAC", "Status"], ifaces.map(i => [
        i.name, i.ip || "-", i.mac || "-", i.is_up ? "UP" : "DOWN",
      ]));
    } catch (e) { ipLocalText = "[ERRO] " + e; }

    try {
      const info = await invoke("get_public_ip_info");
      ipPubText = [
        "IPv4:       " + (info.ipv4 || "-"),
        "IPv6:       " + (info.ipv6 || "-"),
        "Hostname:   " + (info.hostname || "-"),
        "Local:      " + [info.city, info.region, info.country].filter(Boolean).join(", "),
        "Pais:       " + (info.country || "-") + (info.country_code ? " (" + info.country_code + ")" : ""),
        "ISP:        " + (info.isp || "-"),
        "Org:        " + (info.org || "-"),
        "ASN:        " + (info.asn || "-") + (info.as_name ? " - " + info.as_name : ""),
        "Timezone:   " + (info.timezone || "-"),
        "Proxy:      " + (info.is_proxy ? "sim" : "nao"),
        "Datacenter: " + (info.is_hosting ? "sim" : "nao"),
      ].join("\n");
    } catch {
      try { ipPubText = "IP publico: " + (await invoke("get_public_ip")); }
      catch (e) { ipPubText = "[ERRO] " + e; }
    }

    try {
      const d = await invoke("dns_lookup", { host });
      dnsText = "Host: " + d.host + "\nEnderecos:\n" + d.addresses.map(ip => "  - " + ip).join("\n") + "\nReverso: " + (d.reverse || "-");
    } catch (e) { dnsText = "[ERRO] " + e; }

    try {
      const p = await invoke("ping", { host, count: 10 });
      pingText = [
        "Host: " + p.host,
        plainTable(
          ["Enviados", "Recebidos", "Perdidos", "Perda", "Min", "Media", "Max", "Jitter", "Qualidade"],
          [[
            p.packets_sent,
            p.packets_received,
            p.packets_sent - p.packets_received,
            pct(p.loss_pct),
            fmtMs(p.min_ms),
            fmtMs(p.avg_ms),
            fmtMs(p.max_ms),
            fmtMs(p.jitter_ms),
            p.quality,
          ]]
        ),
      ].join("\n");
    } catch (e) { pingText = "[ERRO] " + e; }

    try {
      const h = await invoke("trace_route", { host });
      const replies = h.filter(x => x.address !== "*" && x.status !== "no_reply");
      const noReply = h.length - replies.length;
      tracerouteText = [
        "Destino: " + host,
        "Resumo: " + h.length + " hops, " + replies.length + " com resposta, " + noReply + " sem resposta",
        plainTable(["Hop", "IP", "Hostname", "Min", "Media", "Max", "Perda", "Status"], h.map(x => [
          x.hop_number,
          x.address,
          x.hostname || "-",
          x.min_ms > 0 ? fmtMs(x.min_ms) : "-",
          x.avg_ms > 0 ? fmtMs(x.avg_ms) : "-",
          x.max_ms > 0 ? fmtMs(x.max_ms) : "-",
          pct(x.loss_pct),
          x.status,
        ])),
        noReply >= 3 ? "Observacao: varios hops sem resposta indicam filtragem de ICMP/UDP/TCP ou bloqueio no caminho, nao necessariamente queda de internet." : "",
      ].filter(Boolean).join("\n");
    } catch (e) { tracerouteText = "[ERRO] " + e; }

    try {
      const p = await invoke("get_listening_ports");
      portsText = "Total: " + p.length + " portas em escuta\n" + plainTable(["Porta", "Protocolo", "Estado"], p.map(x => [x.port, x.protocol, x.state]));
    } catch (e) { portsText = "[ERRO] " + e; }

    try {
      const r = await invoke("scan_ports", { host, portsList: portList, timeout_ms: 1500 });
      const open = r.filter(x => x.state === "open");
      const filtered = r.filter(x => x.state === "filtered");
      scanText = [
        "Host: " + host,
        "Resumo: " + r.length + " portas escaneadas, " + open.length + " abertas, " + filtered.length + " filtradas",
        open.length ? plainTable(["Porta", "Servico", "Resposta"], open.map(x => [x.port, x.service, x.response_ms ? x.response_ms.toFixed(0) + " ms" : "-"])) : "Nenhuma porta aberta entre as portas comuns testadas.",
      ].join("\n");
    } catch (e) { scanText = "[ERRO] " + e; }

    try {
      const g = await invoke("get_gateway_info");
      gatewayText = [
        g.warning ? "Aviso: " + g.warning : "Nenhum gateway duplo detectado.",
        plainTable(["IP", "Interface", "Metrica", "Latencia", "Alcancavel", "Primario"], (g.gateways || []).map(x => [
          x.ip, x.interface, x.metric, x.latency_ms !== null ? fmtMs(x.latency_ms) : "-", x.reachable ? "sim" : "nao", x.is_primary ? "sim" : "nao",
        ])),
      ].join("\n");
    } catch (e) { gatewayText = "[ERRO] " + e; }

    try {
      const b = await invoke("benchmark_dns");
      dnsBenchText = plainTable(["Servidor", "IP", "Latencia", "Status", "Melhor"], b.map(x => [
        x.name, x.ip, x.latency_ms ? x.latency_ms + " ms" : "-", x.status, x.best ? "sim" : "nao",
      ]));
    } catch (e) { dnsBenchText = "[ERRO] " + e; }

    try {
      const targets = await invoke("get_http_targets");
      const results = await invoke("test_http_timing", { urls: targets });
      httpText = plainTable(["URL", "Status", "Connect", "TTFB", "Total", "Qualidade"], results.map(t => [
        t.url, t.status_code, fmtMs(t.connect_ms), fmtMs(t.ttfb_ms), fmtMs(t.total_ms), t.quality,
      ]));
    } catch (e) { httpText = "[ERRO] " + e; }

    try {
      const s = await invoke("get_interface_stats");
      ifaceStatsText = plainTable(["Interface", "RX MB", "TX MB", "RX erros", "TX erros", "RX drop"], s.map(x => [
        x.name, x.rx_mb.toFixed(1), x.tx_mb.toFixed(1), x.rx_errors, x.tx_errors, x.rx_dropped,
      ]));
    } catch (e) { ifaceStatsText = "[ERRO] " + e; }

    const report = await invoke("generate_report", {
      ipLocal: ipLocalText, ipPub: ipPubText, dns: dnsText, ping: pingText,
      traceroute: tracerouteText, portsStr: portsText, scan: scanText,
      gateway: gatewayText, dnsBench: dnsBenchText, httpTiming: httpText,
      ifaceStats: ifaceStatsText,
    });

    D("report-output").textContent = report;
    saveToHistory(report);
    toast("Relatório gerado com sucesso", "success");
  });
}

function saveToHistory(text) {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    const h = raw ? JSON.parse(raw) : [];
    h.unshift({ timestamp: new Date().toISOString(), reportText: text });
    if (h.length > 10) h.length = 10;
    localStorage.setItem(HISTORY_KEY, JSON.stringify(h));
  } catch {}
}

function renderHistory() {
  const raw = localStorage.getItem(HISTORY_KEY);
  const h = raw ? JSON.parse(raw) : [];
  historyList.innerHTML = h.length
    ? h.map((item, i) => '<div class="history-item" data-idx="' + i + '"><div class="history-date">' + new Date(item.timestamp).toLocaleString("pt-BR") + '</div><div class="history-summary">' + (item.reportText?.slice(0, 80) || "") + '...</div></div>').join("")
    : '<div style="padding:20px;color:var(--text-dim)">Nenhum relatório salvo.</div>';
  historyList.querySelectorAll(".history-item").forEach(el => {
    el.addEventListener("click", () => {
      const idx = parseInt(el.dataset.idx);
      const data = JSON.parse(localStorage.getItem(HISTORY_KEY))[idx];
      D("report-output").textContent = data.reportText;
      document.querySelector('.nav-item[data-view="report"]').click();
    });
  });
}

// ─── Export / Copy ───
function getOutputText() {
  return D("report-output").textContent || D("report-output").innerText;
}

async function exportReport() {
  const text = getOutputText();
  if (!text.trim()) return;
  const date = new Date().toISOString().slice(0, 19).replace(/[T:-]/g, "");
  try {
    const path = await save({ defaultPath: "relatorio_rede_" + date + ".txt", filters: [{ name: "Texto", extensions: ["txt"] }] });
    if (!path) return;
    const { writeTextFile } = await import("@tauri-apps/plugin-fs");
    await writeTextFile(path, text);
    toast("Exportado: " + path, "success");
  } catch (err) { toast("Erro ao exportar: " + err, "error"); }
}

async function exportHtml() {
  const text = getOutputText();
  if (!text.trim()) return;
  const date = new Date().toISOString().slice(0, 19).replace(/[T:-]/g, "");
  const html = `<!DOCTYPE html>
<html lang="pt-BR"><head><meta charset="UTF-8"><title>KingNetworkTools - Relatorio</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:"Segoe UI",sans-serif;background:#1a1b26;color:#c0caf5;padding:40px;line-height:1.7}
h1{color:#7aa2f7;font-size:1.5rem}
pre{font-family:"JetBrains Mono",monospace;font-size:0.85rem;white-space:pre-wrap;word-break:break-all;background:#24253a;padding:24px;border-radius:8px;border:1px solid #3b4261;margin-top:16px}
.header{color:#565f89;font-size:0.9rem;margin-bottom:24px}
hr{border:none;border-top:1px solid #3b4261;margin:16px 0}
@media print{body{background:#fff;color:#000}pre{background:#f5f5f5;border-color:#ccc}}
</style></head><body>
<h1>KingNetworkTools — Relatório de Diagnóstico</h1>
<div class="header">Gerado em ${new Date().toLocaleString("pt-BR")}</div><hr>
<pre>${text.replace(/</g, "&lt;")}</pre><hr>
<div class="header">KingNetworkTools</div></body></html>`;
  try {
    const path = await save({ defaultPath: "relatorio_rede_" + date + ".html", filters: [{ name: "HTML", extensions: ["html"] }] });
    if (!path) return;
    const { writeTextFile } = await import("@tauri-apps/plugin-fs");
    await writeTextFile(path, html);
    toast("HTML exportado", "success");
  } catch (err) { toast("Erro: " + err, "error"); }
}

function copyOutput() {
  const text = getOutputText();
  if (!text) return;
  navigator.clipboard.writeText(text).then(() => toast("Copiado!", "success"), () => toast("Erro ao copiar", "error"));
}

// ─── Continuous Ping (Event-Streaming) ───
let unlistenPing = null;

async function doContinuousPing() {
  if (continuousPingRunning) return;
  const host = hostInput.value.trim() || "8.8.8.8";
  continuousPingRunning = true;
  pingData = [];
  clearOutput("ping-output");
  appendOutput("ping-output", "  Ping contínuo para " + host + " iniciado...\n");
  D("btn-continuous-ping").disabled = true;
  D("btn-stop-ping").disabled = false;
  graphSection.classList.remove("hidden");
  graphStats.innerHTML = '<span style="color:var(--text-dim)">Aguardando primeiro ping...</span>';

  const canvas = latencyCanvas;
  const ctx = canvas.getContext("2d");
  const W = canvas.width, H = canvas.height;

  function draw() {
    ctx.clearRect(0, 0, W, H);
    if (pingData.length < 2) {
      ctx.fillStyle = "#565f89"; ctx.font = "14px sans-serif"; ctx.textAlign = "center";
      ctx.fillText("Aguardando...", W/2, H/2); return;
    }
    const mx = Math.max(...pingData, 1), mn = Math.min(...pingData, 0), rng = mx - mn || 1;
    const step = W / (pingData.length - 1);
    ctx.beginPath(); ctx.strokeStyle = "#d4a843"; ctx.lineWidth = 2;
    pingData.forEach((v, i) => { const x = i*step, y = H - ((v-mn)/rng)*(H-20)-10; i===0 ? ctx.moveTo(x,y) : ctx.lineTo(x,y); });
    ctx.stroke();
    ctx.fillStyle = "#7a7a9a"; ctx.font = "10px sans-serif";
    ctx.fillText(mx.toFixed(0)+"ms", 2, 12); ctx.fillText(mn.toFixed(0)+"ms", 2, H-4);
  }

  draw();

  function updateStats() {
    if (!pingData.length) return;
    const cur = pingData[pingData.length-1], mn = Math.min(...pingData), mx = Math.max(...pingData);
    const avg = pingData.reduce((a,b)=>a+b,0)/pingData.length;
    graphStats.innerHTML = 'Atual: <span class="' + qualityClass(cur,30,80) + '">' + cur.toFixed(1)+'ms</span>' +
      ' Mín: <span class="' + qualityClass(mn,30,80) + '">' + mn.toFixed(1)+'ms</span>' +
      ' Máx: <span class="' + qualityClass(mx,30,80) + '">' + mx.toFixed(1)+'ms</span>' +
      ' Média: <span class="' + qualityClass(avg,30,80) + '">' + avg.toFixed(1)+'ms</span>';
    draw();
  }

  // Listen for ping events from Rust backend
  unlistenPing = await listen("ping-event", (event) => {
    if (!continuousPingRunning) return;
    const data = event.payload;
    const latency = data.latency_ms ?? data.latencyMs ?? 0;
    const sequence = data.sequence ?? "?";
    const done = data.done ?? false;

    if (data.success && latency > 0) {
      pingData.push(latency);
      if (pingData.length > MAX_PING_POINTS) pingData.shift();
      updateStats();
      appendOutput("ping-output", "  #" + sequence + " resposta de " + host + ": " + latency.toFixed(1) + " ms");
    } else {
      appendOutput("ping-output", '  <span class="error">#' + sequence + " sem resposta</span>");
    }

    if (done) {
      appendOutput("ping-output", "\n  Ping contínuo finalizado.");
      stopContinuousPing();
    }
  });

  // Start the continuous ping backend task
  try {
    await invoke("start_continuous_ping", { host, count: 30, intervalMs: 1000 });
  } catch (e) {
    console.error("Erro ao iniciar ping contínuo:", e);
    toast("Erro ao iniciar ping contínuo: " + e, "error");
    stopContinuousPing();
  }
}

function stopContinuousPing() {
  const wasRunning = continuousPingRunning;
  continuousPingRunning = false;
  if (continuousPingTimer) { clearTimeout(continuousPingTimer); continuousPingTimer = null; }
  if (unlistenPing) { unlistenPing(); unlistenPing = null; }
  graphSection.classList.add("hidden");
  D("btn-continuous-ping").disabled = false;
  D("btn-stop-ping").disabled = true;
  if (wasRunning) toast("Ping contínuo finalizado", "info");
}

// ─── Load port list ───
async function loadPortList() {
  try { portList = await invoke("get_port_list"); }
  catch { portList = [21,22,23,25,53,80,110,111,135,139,143,443,445,465,587,993,995,1080,1194,1433,1521,2049,2375,3000,3306,3389,4444,5432,5900,6379,6881,7070,8080,8443,8888,9000,9090,9200,10000,11211,27017,27018,50000,51413,52869,55443,60000]; }
}

// ─── Init ───
document.addEventListener("DOMContentLoaded", () => {
  loadDashboard();
  loadPortList();

  // Sidebar nav
  document.querySelectorAll(".nav-item[data-view]").forEach(btn => {
    btn.addEventListener("click", () => {
      document.querySelectorAll(".nav-item").forEach(b => b.classList.remove("active"));
      btn.classList.add("active");
      document.querySelectorAll(".view").forEach(v => v.classList.remove("active"));
      const v = D("view-" + btn.dataset.view);
      if (v) v.classList.add("active");
      D("view-title").textContent = btn.querySelector("span:last-child").textContent;
    });
  });

  // Tool buttons
  D("btn-ping").addEventListener("click", showPing);
  D("btn-traceroute").addEventListener("click", showTraceroute);
  D("btn-mtr").addEventListener("click", showMtr);
  D("btn-local-ip").addEventListener("click", showLocalIp);
  D("btn-public-ip").addEventListener("click", showPublicIp);
  D("btn-public-ip-info").addEventListener("click", showPublicIpInfo);
  D("btn-gateway").addEventListener("click", showGateway);
  D("btn-dns").addEventListener("click", showDnsLookup);
  D("btn-dns-bench").addEventListener("click", showDnsBench);
  D("btn-listening").addEventListener("click", showListeningPorts);
  D("btn-scan").addEventListener("click", showPortScan);
  D("btn-scan-network").addEventListener("click", showNetworkScan);
  D("btn-http").addEventListener("click", showHttpTiming);
  D("btn-report").addEventListener("click", showFullReport);
  D("btn-dashboard-report").addEventListener("click", () => {
    document.querySelector('.nav-item[data-view="report"]').click();
    showFullReport();
  });

  D("btn-export").addEventListener("click", exportReport);
  D("btn-export-html").addEventListener("click", exportHtml);
  D("btn-copy").addEventListener("click", copyOutput);
  D("btn-clear").addEventListener("click", () => D("report-output").innerHTML = "");

  D("btn-continuous-ping").addEventListener("click", doContinuousPing);
  D("btn-stop-ping").addEventListener("click", stopContinuousPing);

  D("btn-history").addEventListener("click", () => {
    const show = historySection.classList.toggle("hidden");
    if (!show) renderHistory();
  });
  D("btn-close-history").addEventListener("click", () => historySection.classList.add("hidden"));
  D("btn-clear-history").addEventListener("click", () => { localStorage.removeItem(HISTORY_KEY); renderHistory(); });

  D("btn-theme").addEventListener("click", () => {
    document.body.classList.toggle("light");
    const isLight = document.body.classList.contains("light");
    D("btn-theme").innerHTML = isLight ? '\u2600\uFE0F' : '\uD83C\uDF19';
    localStorage.setItem("kinganaliser_theme", isLight ? "light" : "dark");
  });

  // Apply saved theme
  if (localStorage.getItem("kinganaliser_theme") === "light") {
    document.body.classList.add("light");
    D("btn-theme").innerHTML = "\u2600\uFE0F";
  }

  // Enter key
  hostInput.addEventListener("keydown", e => { if (e.key === "Enter") showPing(); });
  trHost.addEventListener("keydown", e => { if (e.key === "Enter") showTraceroute(); });
  dnsInput.addEventListener("keydown", e => { if (e.key === "Enter") showDnsLookup(); });

  // Timeout slider
  if (scanTimeout) {
    scanTimeout.addEventListener("input", () => { scanTimeoutLabel.textContent = scanTimeout.value + "ms"; });
  }
});
