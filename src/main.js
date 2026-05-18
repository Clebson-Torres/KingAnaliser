import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

const hostInput = document.getElementById("host-input");
const dnsInput = document.getElementById("dns-input");
const subnetInput = document.getElementById("subnet-input");
const ifaceSelect = document.getElementById("iface-select");
const outputContent = document.getElementById("output-content");
const outputSection = document.getElementById("output");
const statusBar = document.getElementById("status-bar");
const statusText = document.getElementById("status-text");
const hostError = document.getElementById("host-error");
const dnsError = document.getElementById("dns-error");
const graphSection = document.getElementById("graph-section");
const latencyCanvas = document.getElementById("latency-canvas");
const graphCurrent = document.getElementById("graph-current");
const graphMin = document.getElementById("graph-min");
const graphMax = document.getElementById("graph-max");
const graphAvg = document.getElementById("graph-avg");
const historySection = document.getElementById("history-section");
const historyList = document.getElementById("history-list");
const gatewayWarning = document.getElementById("gateway-warning");
const scanTimeoutSlider = document.getElementById("scan-timeout");
const scanTimeoutLabel = document.getElementById("scan-timeout-label");
const scanProgress = document.getElementById("scan-progress");
const scanProgressFill = document.getElementById("scan-progress-fill");
const scanProgressText = document.getElementById("scan-progress-text");

let portList = [];
let continuousPingRunning = false;
let continuousPingTimer = null;
let pingData = [];
const MAX_PING_POINTS = 60;
let isRunning = {};

const HISTORY_KEY = "kinganaliser_history";

function showStatus(msg) {
  statusBar.classList.remove("hidden");
  statusText.textContent = msg;
}

function hideStatus() {
  statusBar.classList.add("hidden");
}

function showOutput() {
  outputSection.classList.remove("hidden");
  historySection.classList.add("hidden");
}

function showHistory() {
  historySection.classList.remove("hidden");
  outputSection.classList.add("hidden");
}

function clearOutput() {
  outputContent.textContent = "";
}

function appendOutput(text) {
  outputContent.innerHTML += text + "\n";
  outputContent.scrollTop = outputContent.scrollHeight;
}

function appendSection(title) {
  appendOutput("");
  const line = "\u2500".repeat(Math.min(50, title.length + 10));
  appendOutput(line);
  appendOutput("  " + title);
  appendOutput(line);
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

function validateIP(str) {
  const re = /^(\d{1,3}\.){3}\d{1,3}$/;
  if (!re.test(str)) return false;
  return str.split(".").every(n => { const v = parseInt(n, 10); return v >= 0 && v <= 255; });
}

function validateDomain(str) {
  return /^[a-zA-Z0-9][a-zA-Z0-9\-\.]+\.[a-zA-Z]{2,}$/.test(str) || validateIP(str);
}

function validateHostInput() {
  const v = hostInput.value.trim();
  if (!v) { hostError.textContent = "Preencha o campo de host"; return false; }
  if (!validateDomain(v) && !validateIP(v)) { hostError.textContent = "Insira um IP ou domínio válido"; return false; }
  hostError.textContent = "";
  return true;
}

function validateDnsInput() {
  const v = dnsInput.value.trim();
  if (!v) { dnsError.textContent = "Preencha o campo DNS"; return false; }
  if (!validateDomain(v) && !validateIP(v)) { dnsError.textContent = "Insira um IP ou domínio válido"; return false; }
  dnsError.textContent = "";
  return true;
}

function setRunning(key, running) {
  isRunning[key] = running;
  const btn = document.getElementById("btn-" + key);
  if (btn) btn.disabled = running;
}

function isBusy(key) {
  return !!isRunning[key];
}

async function withLoading(msg, key, fn) {
  if (isBusy(key)) return;
  setRunning(key, true);
  showStatus(msg);
  showOutput();
  try {
    const result = await fn();
    return result;
  } catch (err) {
    catchError(err);
  } finally {
    setRunning(key, false);
    hideStatus();
  }
}

function catchError(err) {
  appendOutput('<span class="error">[ERRO] ' + err + "</span>");
  console.error(err);
}

async function loadInterfaces() {
  try {
    const interfaces = await invoke("get_network_interfaces");
    ifaceSelect.innerHTML = "";
    const emptyOpt = document.createElement("option");
    emptyOpt.value = "";
    emptyOpt.textContent = "-- Selecione uma interface --";
    ifaceSelect.appendChild(emptyOpt);
    for (const iface of interfaces) {
      const opt = document.createElement("option");
      opt.value = iface.ip;
      const statusIcon = iface.is_up ? "\u2713" : "\u2717";
      const statusClass = iface.is_up ? "quality-good" : "quality-bad";
      opt.innerHTML = iface.name + " (" + iface.ip + ")" + (iface.mac ? " - " + iface.mac : "") + ' <span class="' + statusClass + '">' + statusIcon + "</span>";
      ifaceSelect.appendChild(opt);
    }
  } catch (e) {
    console.error("Erro ao carregar interfaces:", e);
  }
}

async function loadPortList() {
  try {
    portList = await invoke("get_port_list");
  } catch (e) {
    console.error("Erro ao carregar lista de portas:", e);
    portList = [21,22,23,25,53,80,110,111,135,139,143,443,445,465,587,993,995,1080,1194,1433,1521,2049,2375,3000,3306,3389,4444,5432,5900,6379,6881,7070,8080,8443,8888,9000,9090,9200,10000,11211,27017,27018,50000,51413,52869,55443,60000];
  }
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

async function showLocalIp() {
  await withLoading("Obtendo IP local...", "local-ip", async () => {
    appendSection("Interfaces de Rede");
    const interfaces = await invoke("get_network_interfaces");
    const rows = interfaces.map(i => {
      const statusCls = i.is_up ? "quality-good" : "quality-bad";
      return ['<span class="' + statusCls + '">' + i.name + "</span>", i.ip, i.mac || "-", i.is_up ? "Ativa" : "Inativa"];
    });
    appendOutput(toTable(["Interface", "IP", "MAC", "Status"], rows));
  }).catch(catchError);
}

async function showPublicIp() {
  await withLoading("Consultando IP público...", "public-ip", async () => {
    appendSection("IP Público");
    const ip = await invoke("get_public_ip");
    appendOutput("  IP Público: " + ip);
  }).catch(catchError);
}

async function showPublicIpInfo() {
  await withLoading("Consultando geolocalização...", "public-ip-info", async () => {
    appendSection("Geolocalização do IP Público");
    const info = await invoke("get_public_ip_info");
    appendOutput("  IPv4:      " + info.ipv4);
    appendOutput("  IPv6:      " + (info.ipv6 || "N/A"));
    appendOutput("  Hostname:  " + (info.hostname || "N/A"));
    appendOutput("  País:      " + info.country + " (" + info.country_code + ")");
    appendOutput("  Região:    " + info.region);
    appendOutput("  Cidade:    " + info.city);
    appendOutput("  ISP:       " + info.isp);
    appendOutput("  Org:       " + info.org);
    appendOutput("  ASN:       " + info.asn + " - " + info.as_name);
    appendOutput("  Proxy:     " + (info.is_proxy ? "Sim" : "Não"));
    appendOutput("  Hosting:   " + (info.is_hosting ? "Sim" : "Não"));
  }).catch(catchError);
}

async function showDnsLookup() {
  if (!validateDnsInput()) return;
  const host = dnsInput.value.trim();
  await withLoading("Resolvendo DNS para " + host + "...", "dns", async () => {
    appendSection("DNS Lookup: " + host);
    const result = await invoke("dns_lookup", { host });
    appendOutput("  Host:     " + result.host);
    appendOutput("  IPs:");
    for (const ip of result.addresses) {
      appendOutput("    - " + ip);
    }
    appendOutput("  Reverso:  " + (result.reverse || "(não disponível)"));
  }).catch(catchError);
}

async function showDnsBench() {
  await withLoading("Executando benchmark DNS...", "dns-bench", async () => {
    appendSection("DNS Benchmark");
    appendOutput("  Testando servidores DNS... (pode levar alguns segundos)\n");
    const results = await invoke("benchmark_dns");
    if (results.length === 0) {
      appendOutput("  Nenhum resultado obtido.");
      return;
    }
    const rows = results.map(r => {
      const latCls = qualityClass(r.latency_ms, 30, 80);
      const bestMark = r.best ? ' <span class="quality-good">★ Mais rápido</span>' : "";
      return [
        '<span class="server-name">' + r.name + "</span>" + bestMark,
        r.ip,
        '<span class="' + latCls + '">' + r.latency_ms + "ms</span>",
        r.status === "OK" ? '<span class="quality-good">OK</span>' : '<span class="quality-bad">' + r.status + "</span>",
      ];
    });
    appendOutput(toTable(["Servidor", "IP", "Latência", "Status"], rows));
  }).catch(catchError);
}

async function showPing() {
  if (!validateHostInput()) return;
  const host = hostInput.value.trim();
  await withLoading("Executando ping para " + host + "...", "ping", async () => {
    appendSection("Ping: " + host);
    const result = await invoke("ping", { host, count: 10 });

    appendOutput(toTable(
      ["Pacotes Enviados", "Recebidos", "Perdidos", "Perda%"],
      [[
        String(result.packets_sent),
        String(result.packets_received),
        String(result.packets_sent - result.packets_received),
        '<span class="' + (result.loss_pct === 0 ? "quality-good" : result.loss_pct <= 2 ? "quality-warn" : "quality-bad") + '">' + result.loss_pct.toFixed(1) + "%</span>"
      ]]
    ));

    if (result.packets_received > 0) {
      appendOutput(toTable(
        ["Mínimo", "Média", "Máximo", "Jitter"],
        [[
          '<span class="' + qualityClass(result.min_ms, 30, 80) + '">' + result.min_ms.toFixed(1) + " ms</span>",
          '<span class="' + qualityClass(result.avg_ms, 30, 80) + '">' + result.avg_ms.toFixed(1) + " ms</span>",
          '<span class="' + qualityClass(result.max_ms, 30, 80) + '">' + result.max_ms.toFixed(1) + " ms</span>",
          '<span class="' + (result.jitter_ms < 5 ? "quality-good" : result.jitter_ms < 20 ? "quality-warn" : "quality-bad") + '">' + result.jitter_ms.toFixed(1) + " ms</span>"
        ]]
      ));
    }

    appendOutput('  Avaliação: ' + badge(result.quality, result.quality_color));
  }).catch(catchError);
}

async function showTraceroute() {
  if (!validateHostInput()) return;
  const host = hostInput.value.trim();
  await withLoading("Traçando rota para " + host + "...", "traceroute", async () => {
    appendSection("Rota até " + host);
    const hops = await invoke("trace_route", { host });

    const rows = hops.map(h => {
      const statusCls = h.status === "ok" ? "quality-good" : h.status === "warning" ? "quality-warn" : "quality-bad";
      const perdaCls = h.loss_pct === 0 ? "quality-good" : h.loss_pct <= 5 ? "quality-warn" : "quality-bad";
      const criticalClass = h.status === "critical" ? ' class="highlight-critical"' : "";
      const hostname = h.hostname || "-";
      return [
        '<span' + criticalClass + '>' + String(h.hop_number) + "</span>",
        '<span' + criticalClass + '>' + h.address + "</span>",
        hostname,
        h.min_ms > 0 ? h.min_ms.toFixed(1) + "ms" : "-",
        h.avg_ms > 0 ? '<span class="' + qualityClass(h.avg_ms, 30, 80) + '">' + h.avg_ms.toFixed(1) + "ms</span>" : "-",
        h.max_ms > 0 ? h.max_ms.toFixed(1) + "ms" : "-",
        '<span class="' + perdaCls + '">' + h.loss_pct.toFixed(0) + "%</span>",
        '<span class="' + statusCls + '">' + h.status + "</span>",
      ];
    });

    appendOutput(toTable(
      ["#", "IP", "Hostname", "Min", "Avg", "Max", "Perda%", "Status"],
      rows
    ));

    const critical = hops.filter(h => h.status === "critical");
    if (critical.length > 0) {
      appendOutput('\n  <span class="quality-bad">⚠ ' + critical.length + " hop(s) com status CRÍTICO detectado(s):</span>");
      for (const h of critical) {
        appendOutput('    → Hop ' + h.hop_number + " (" + h.address + ") — avg " + h.avg_ms.toFixed(1) + "ms");
      }
    }
  }).catch(catchError);
}

async function showMtr() {
  if (!validateHostInput()) return;
  const host = hostInput.value.trim();
  await withLoading("Executando MTR para " + host + "...", "mtr", async () => {
    appendSection("MTR: " + host);
    appendOutput("  Executando MTR com 5 ciclos... (pode levar alguns segundos)\n");
    const hops = await invoke("run_mtr", { host, cycles: 5 });
    const rows = hops.map(h => {
      const avgCls = qualityClass(h.avg_ms, 30, 80);
      const lossCls = h.loss_pct === 0 ? "quality-good" : (h.loss_pct <= 2 ? "quality-warn" : "quality-bad");
      const qualCls = h.quality === "ok" ? "quality-good" : (h.quality === "warning" ? "quality-warn" : "quality-bad");
      return [
        String(h.hop),
        h.host,
        '<span class="' + lossCls + '">' + h.loss_pct.toFixed(1) + "%</span>",
        '<span class="' + avgCls + '">' + h.avg_ms.toFixed(1) + "ms</span>",
        h.best_ms.toFixed(1) + "ms",
        h.worst_ms.toFixed(1) + "ms",
        h.jitter_ms.toFixed(1) + "ms",
        '<span class="' + qualCls + '">' + h.quality + "</span>",
      ];
    });
    appendOutput(toTable(["Hop", "Host", "Perda", "Média", "Melhor", "Pior", "Jitter", "Qualid."], rows));
  }).catch(catchError);
}

async function showGateway() {
  await withLoading("Obtendo informações do gateway...", "gateway", async () => {
    appendSection("Gateway");
    const info = await invoke("get_gateway_info");

    gatewayWarning.classList.add("hidden");

    if (info.has_multiple) {
      gatewayWarning.textContent = "⚠ " + (info.warning || "Gateway duplo detectado");
      gatewayWarning.classList.remove("hidden");
    }

    const rows = info.gateways.map(g => {
      const latCls = g.latency_ms !== null ? qualityClass(g.latency_ms, 30, 80) : "";
      const reachCls = g.reachable ? "quality-good" : "quality-bad";
      return [
        g.ip,
        g.interface,
        String(g.metric),
        g.latency_ms !== null ? '<span class="' + latCls + '">' + g.latency_ms.toFixed(1) + " ms</span>" : "N/A",
        '<span class="' + reachCls + '">' + (g.reachable ? "Sim" : "Não") + "</span>",
        g.is_primary ? '<span class="quality-good">★ Sim</span>' : "Não",
      ];
    });

    appendOutput(toTable(
      ["IP", "Interface", "Métrica", "Latência", "Alcançável", "Primário"],
      rows
    ));
  }).catch(catchError);
}

async function showHttpTiming() {
  await withLoading("Testando timing HTTP...", "http", async () => {
    appendSection("HTTP Timing");
    const targets = await invoke("get_http_targets");
    const results = await invoke("test_http_timing", { urls: targets });
    for (const t of results) {
      appendOutput("  " + t.url + ":");
      const qualCls = t.quality === "ok" ? "quality-good" : (t.quality === "slow" ? "quality-warn" : "quality-bad");
      appendOutput(toTable(
        ["DNS", "Connect", "TTFB", "Total", "Status", "Qualid."],
        [[
          '<span class="' + qualityClass(t.dns_ms, 50, 200) + '">' + t.dns_ms.toFixed(1) + "ms</span>",
          '<span class="' + qualityClass(t.connect_ms, 100, 300) + '">' + t.connect_ms.toFixed(1) + "ms</span>",
          '<span class="' + qualityClass(t.ttfb_ms, 150, 400) + '">' + t.ttfb_ms.toFixed(1) + "ms</span>",
          '<span class="' + qualityClass(t.total_ms, 200, 500) + '">' + t.total_ms.toFixed(1) + "ms</span>",
          String(t.status_code),
          '<span class="' + qualCls + '">' + t.quality + "</span>",
        ]]
      ));
    }
  }).catch(catchError);
}

async function showListeningPorts() {
  await withLoading("Obtendo portas em escuta...", "listening", async () => {
    appendSection("Portas em Escuta");
    const ports = await invoke("get_listening_ports");
    if (ports.length === 0) {
      appendOutput("  Nenhuma porta em escuta encontrada.");
    } else {
      const rows = ports.map(p => [String(p.port), p.protocol, p.state]);
      appendOutput(toTable(["Porta", "Protocolo", "Estado"], rows));
      appendOutput("\n  Total: " + ports.length + " portas");
    }
  }).catch(catchError);
}

async function showPortScan() {
  if (!validateHostInput()) return;
  const host = hostInput.value.trim() || "127.0.0.1";
  const timeout = parseInt(scanTimeoutSlider.value, 10);
  await withLoading("Escaneando portas em " + host + " (timeout " + timeout + "ms)...", "scan", async () => {
    appendSection("Scan de Portas: " + host);
    appendOutput("  Timeout configurado: " + timeout + "ms\n");
    const results = await invoke("scan_ports", { host, portsList: portList, timeout_ms: timeout });

    const open = results.filter(r => r.state === "open");
    const filtered = results.filter(r => r.state === "filtered");
    const closed = results.filter(r => r.state === "closed");

    if (open.length > 0) {
      appendOutput("\n  ABERTAS (" + open.length + "):");
      const openRows = open.map(r => [
        String(r.port),
        r.service,
        '<span class="quality-good">aberta</span>',
        r.response_ms !== null ? r.response_ms.toFixed(0) + "ms" : "-"
      ]);
      appendOutput(toTable(["Porta", "Serviço", "Estado", "Resposta"], openRows));
    }

    if (filtered.length > 0) {
      appendOutput("\n  FILTRADAS (" + filtered.length + ") — sem resposta dentro do timeout:");
      const fNames = filtered.map(r => r.port + " (" + r.service + ")").join(", ");
      appendOutput("    " + fNames);
    }

    if (closed.length > 0) {
      appendOutput("\n  FECHADAS (" + closed.length + ") — conexão recusada:");
      const cNames = closed.map(r => r.port + " (" + r.service + ")").join(", ");
      appendOutput("    " + cNames);
    }

    appendOutput("\n  Escaneadas: " + results.length + " | Abertas: " + open.length + " | Filtradas: " + filtered.length + " | Fechadas: " + closed.length);
  }).catch(catchError);
}

async function showNetworkScan() {
  await withLoading("Escaneando rede...", "scan-network", async () => {
    appendSection("Scan de Rede");
    const subnet = subnetInput.value.trim() || null;

    appendOutput("  Sub-rede: " + (subnet || "detecção automática") + "\n");

    scanProgress.classList.remove("hidden");
    scanProgressFill.style.width = "0%";
    scanProgressText.textContent = "Iniciando scan...";

    const result = await invoke("scan_network", { subnet });

    scanProgress.classList.add("hidden");

    appendOutput("  Sub-rede escaneada : " + result.subnet);
    appendOutput("  Hosts encontrados  : " + result.hosts_up + " de " + result.total_hosts);
    appendOutput("  Duração do scan    : " + result.scan_duration_secs.toFixed(1) + "s");

    if (result.hosts.length === 0) {
      appendOutput("\n  Nenhum host ativo encontrado.");
    } else {
      const rows = result.hosts.map(h => {
        const gwMark = h.is_gateway ? ' <span class="quality-good">[Gateway]</span>' : "";
        const mac = h.mac || "-";
        const vendor = h.vendor || "-";
        const latency = h.latency_ms !== null ? h.latency_ms.toFixed(1) + "ms" : "-";
        const ports = h.open_ports.length > 0 ? h.open_ports.join(", ") : "-";
        const hostname = h.hostname || "-";
        return [
          h.ip + gwMark,
          hostname,
          mac,
          vendor,
          latency,
          ports,
        ];
      });

      appendOutput(toTable(
        ["IP", "Hostname", "MAC", "Fabricante", "Latência", "Portas abertas"],
        rows
      ));
    }
  }).catch(catchError);
}

async function showIfaceStats() {
  await withLoading("Obtendo estatísticas de interface...", "iface-stats", async () => {
    appendSection("Estatísticas de Interface");
    const stats = await invoke("get_interface_stats");
    if (stats.length === 0) {
      appendOutput("  Nenhuma estatística disponível.");
      return;
    }
    const rows = stats.map(s => {
      const rxErrCls = s.rx_errors > 0 ? "quality-warn" : "quality-good";
      const txErrCls = s.tx_errors > 0 ? "quality-warn" : "quality-good";
      const dropCls = s.rx_dropped > 0 ? "quality-warn" : "quality-good";
      return [
        s.name,
        s.rx_mb.toFixed(2) + " MB",
        s.tx_mb.toFixed(2) + " MB",
        '<span class="' + rxErrCls + '">' + s.rx_errors + "</span>",
        '<span class="' + txErrCls + '">' + s.tx_errors + "</span>",
        '<span class="' + dropCls + '">' + s.rx_dropped + "</span>",
      ];
    });
    appendOutput(toTable(["Interface", "Recebido", "Enviado", "Err RX", "Err TX", "Drop RX"], rows));
  }).catch(catchError);
}

async function doContinuousPing() {
  if (!validateHostInput()) return;
  if (continuousPingRunning) return;
  const host = hostInput.value.trim();
  continuousPingRunning = true;
  pingData = [];
  document.getElementById("btn-continuous-ping").disabled = true;
  document.getElementById("btn-stop-ping").disabled = false;
  graphSection.classList.remove("hidden");

  const canvas = latencyCanvas;
  const ctx = canvas.getContext("2d");
  const W = canvas.width;
  const H = canvas.height;

  function drawGraph() {
    ctx.clearRect(0, 0, W, H);
    if (pingData.length < 2) {
      ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue("--text-dim").trim() || "#565f89";
      ctx.font = "14px sans-serif";
      ctx.textAlign = "center";
      ctx.fillText("Aguardando dados...", W / 2, H / 2);
      return;
    }
    const maxVal = Math.max(...pingData, 1);
    const minVal = Math.min(...pingData, 0);
    const range = maxVal - minVal || 1;
    const step = W / (pingData.length - 1);

    ctx.beginPath();
    ctx.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() || "#7aa2f7";
    ctx.lineWidth = 2;
    for (let i = 0; i < pingData.length; i++) {
      const x = i * step;
      const y = H - ((pingData[i] - minVal) / range) * (H - 20) - 10;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();

    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue("--text-dim").trim() || "#565f89";
    ctx.font = "10px sans-serif";
    ctx.fillText(maxVal.toFixed(0) + "ms", 2, 12);
    ctx.fillText(minVal.toFixed(0) + "ms", 2, H - 4);
  }

  function updateStats() {
    if (pingData.length === 0) return;
    const current = pingData[pingData.length - 1];
    const min = Math.min(...pingData);
    const max = Math.max(...pingData);
    const avg = pingData.reduce((a, b) => a + b, 0) / pingData.length;
    const curCls = qualityClass(current, 30, 80);
    graphCurrent.innerHTML = 'Atual: <span class="' + curCls + '">' + current.toFixed(1) + "ms</span>";
    graphMin.innerHTML = 'Mín: <span class="' + qualityClass(min, 30, 80) + '">' + min.toFixed(1) + "ms</span>";
    graphMax.innerHTML = 'Máx: <span class="' + qualityClass(max, 30, 80) + '">' + max.toFixed(1) + "ms</span>";
    graphAvg.innerHTML = 'Média: <span class="' + qualityClass(avg, 30, 80) + '">' + avg.toFixed(1) + "ms</span>";
    drawGraph();
  }

  async function pingOnce() {
    if (!continuousPingRunning) return;
    try {
      const result = await invoke("ping", { host, count: 1 });
      if (result.packets_received > 0) {
        pingData.push(result.avg_ms || result.min_ms);
        if (pingData.length > MAX_PING_POINTS) {
          pingData.shift();
        }
        updateStats();
      }
    } catch (e) {
      console.error("Continuous ping error:", e);
    }
    if (continuousPingRunning) {
      continuousPingTimer = setTimeout(pingOnce, 1000);
    }
  }

  pingOnce();
}

function stopContinuousPing() {
  continuousPingRunning = false;
  if (continuousPingTimer) {
    clearTimeout(continuousPingTimer);
    continuousPingTimer = null;
  }
  document.getElementById("btn-continuous-ping").disabled = false;
  document.getElementById("btn-stop-ping").disabled = true;
}

async function showFullReport() {
  await withLoading("Gerando relatório completo...", "report", async () => {
    const host = hostInput.value.trim() || "8.8.8.8";

    appendSection("Coletando dados...");

    let ipLocalText = "", ipPubText = "", dnsText = "", pingText = "",
        tracerouteText = "", portsText = "", scanText = "",
        gatewayText = "", dnsBenchText = "", httpText = "", ifaceStatsText = "";

    try {
      const ifaces = await invoke("get_network_interfaces");
      ipLocalText = "  Interface  | IP              | MAC               | Status\n" +
                    "  -----------+-----------------+-------------------+-------\n" +
                    ifaces.map(i => "  " + i.name.padEnd(10) + " | " + i.ip.padEnd(15) + " | " + (i.mac || "-").padEnd(17) + " | " + (i.is_up ? "UP" : "DOWN")).join("\n");
    } catch (e) { ipLocalText = "  [ERRO] " + e; }

    try {
      const info = await invoke("get_public_ip_info");
      ipPubText = "  IPv4          : " + info.ipv4 + "\n" +
                  "  IPv6          : " + (info.ipv6 || "N/A") + "\n" +
                  "  País          : " + info.country + " (" + info.country_code + ")\n" +
                  "  Estado        : " + info.region + "\n" +
                  "  Cidade        : " + info.city + "\n" +
                  "  ISP           : " + info.isp + "\n" +
                  "  Organização   : " + info.org + "\n" +
                  "  ASN           : " + info.asn + " — " + info.as_name + "\n" +
                  "  Proxy/VPN     : " + (info.is_proxy ? "Sim" : "Não") + "\n" +
                  "  Datacenter    : " + (info.is_hosting ? "Sim" : "Não");
    } catch (e) {
      try { ipPubText = "  " + (await invoke("get_public_ip")); }
      catch (e2) { ipPubText = "  [ERRO] " + e; }
    }

    try {
      const d = await invoke("dns_lookup", { host });
      dnsText = "  " + d.host + " -> " + d.addresses.join(", ");
    } catch (e) { dnsText = "  [ERRO] " + e; }

    try {
      const p = await invoke("ping", { host, count: 10 });
      pingText = "  Pacotes enviados   : " + p.packets_sent + "\n" +
                 "  Pacotes recebidos  : " + p.packets_received + "\n" +
                 "  Pacotes perdidos   : " + (p.packets_sent - p.packets_received) + " (" + p.loss_pct.toFixed(1) + "%)\n\n" +
                 "  Latência mínima    : " + p.min_ms.toFixed(1) + "ms\n" +
                 "  Latência média     : " + p.avg_ms.toFixed(1) + "ms\n" +
                 "  Latência máxima    : " + p.max_ms.toFixed(1) + "ms\n" +
                 "  Jitter (mdev)      : " + p.jitter_ms.toFixed(1) + "ms\n\n" +
                 "  Avaliação          : " + p.quality + " — " + (p.quality_color === "green" ? "conexão estável" : p.quality_color === "yellow" ? "conexão com variação" : "conexão instável");
    } catch (e) { pingText = "  [ERRO] " + e; }

    try {
      const h = await invoke("trace_route", { host });
      tracerouteText = "  " + h.length + " hops (até " + host + ")\n";
      tracerouteText += "  Hop | IP              | Min    | Avg    | Max    | Perda | Status\n";
      tracerouteText += "  ----+-----------------+--------+--------+--------+-------+--------\n";
      for (const hop of h) {
        tracerouteText += "  " +
          String(hop.hop_number).padStart(3) + " | " +
          hop.address.padEnd(15) + " | " +
          (hop.min_ms > 0 ? hop.min_ms.toFixed(1).padStart(6) : "     -") + " | " +
          (hop.avg_ms > 0 ? hop.avg_ms.toFixed(1).padStart(6) : "     -") + " | " +
          (hop.max_ms > 0 ? hop.max_ms.toFixed(1).padStart(6) : "     -") + " | " +
          hop.loss_pct.toFixed(0).padStart(4) + "% | " +
          hop.status + "\n";
      }
    } catch (e) { tracerouteText = "  [ERRO] " + e; }

    try {
      const p = await invoke("get_listening_ports");
      portsText = "  Porta | Protocolo\n  ------+----------\n" + p.map(x => "  " + String(x.port).padStart(5) + " | " + x.protocol).join("\n") + "\n\n  Total: " + p.length + " portas";
    } catch (e) { portsText = "  [ERRO] " + e; }

    try {
      const r = await invoke("scan_ports", { host, portsList: portList, timeout_ms: 1500 });
      const open = r.filter(x => x.state === "open");
      const filtered = r.filter(x => x.state === "filtered");
      const closed = r.filter(x => x.state === "closed");
      scanText = "  Timeout configurado: 1500ms\n  Portas escaneadas  : " + r.length + "\n\n  ABERTAS (" + open.length + "):\n";
      scanText += "    Porta | Serviço  | Resposta\n    ------+----------+---------\n";
      for (const o of open) {
        scanText += "    " + String(o.port).padStart(5) + " | " + o.service.padEnd(8) + " | " + (o.response_ms ? o.response_ms.toFixed(0) + "ms" : "-") + "\n";
      }
      if (filtered.length > 0) {
        scanText += "\n  FILTRADAS (" + filtered.length + "): " + filtered.map(x => x.port + " (" + x.service + ")").join(", ") + "\n";
      }
      if (closed.length > 0) {
        scanText += "\n  FECHADAS (" + closed.length + "): " + closed.map(x => x.port + " (" + x.service + ")").join(", ") + "\n";
      }
    } catch (e) { scanText = "  [ERRO] " + e; }

    try {
      const g = await invoke("get_gateway_info");
      gatewayText = "  IP             | Interface | Métrica | Latência | Alcançável | Primário\n";
      gatewayText += "  ---------------+-----------+---------+----------+------------+---------\n";
      for (const gw of g.gateways) {
        gatewayText += "  " + gw.ip.padEnd(14) + " | " + gw.interface.padEnd(9) + " | " +
          String(gw.metric).padStart(7) + " | " +
          (gw.latency_ms !== null ? gw.latency_ms.toFixed(1).padStart(7) + "ms" : "    N/A") + " | " +
          (gw.reachable ? "Sim     " : "Não     ") + " | " +
          (gw.is_primary ? "Sim" : "Não") + "\n";
      }
      if (g.has_multiple) {
        gatewayText += "\n  [AVISO] " + (g.warning || "Gateway duplo detectado") + "\n";
      }
    } catch (e) { gatewayText = "  [ERRO] " + e; }

    try {
      const b = await invoke("benchmark_dns");
      dnsBenchText = "  Servidor          | Nome          | Latência | Status\n";
      dnsBenchText += "  ------------------+---------------+----------+--------\n";
      for (const s of b) {
        dnsBenchText += "  " + s.ip.padEnd(18) + " | " + s.name.padEnd(13) + " | " +
          String(s.latency_ms).padStart(6) + "ms | " + s.status + (s.best ? " ★" : "") + "\n";
      }
    } catch (e) { dnsBenchText = "  [ERRO] " + e; }

    try {
      const targets = await invoke("get_http_targets");
      const results = await invoke("test_http_timing", { urls: targets });
      httpText = "  " + results.map(t => t.url + ": " + t.total_ms.toFixed(0) + "ms (" + t.quality + ")").join("\n  ");
    } catch (e) { httpText = "  [ERRO] " + e; }

    try {
      const s = await invoke("get_interface_stats");
      ifaceStatsText = "  Interface | RX Total  | TX Total  | Erros RX | Erros TX | Drop RX | Drop TX\n" +
        "  ----------+-----------+-----------+----------+----------+---------+---------\n" +
        s.map(x => "  " + x.name.padEnd(8) + " | " +
          x.rx_mb.toFixed(1).padStart(8) + " MB | " +
          x.tx_mb.toFixed(1).padStart(8) + " MB | " +
          String(x.rx_errors).padStart(8) + " | " +
          String(x.tx_errors).padStart(8) + " | " +
          String(x.rx_dropped).padStart(7) + " | 0"
        ).join("\n");
    } catch (e) { ifaceStatsText = "  [ERRO] " + e; }

    const report = await invoke("generate_report", {
      ipLocal: ipLocalText,
      ipPub: ipPubText,
      dns: dnsText,
      ping: pingText,
      traceroute: tracerouteText,
      portsStr: portsText,
      scan: scanText,
      gateway: gatewayText,
      dnsBench: dnsBenchText,
      httpTiming: httpText,
      ifaceStats: ifaceStatsText,
    });

    outputContent.textContent = report;
    outputContent.scrollTop = 0;

    saveToHistory(report);
  }).catch(catchError);
}

function copyOutput() {
  const text = outputContent.textContent || outputContent.innerText;
  navigator.clipboard.writeText(text).then(
    () => {
      const btn = document.getElementById("btn-copy");
      btn.textContent = "\u2705 Copiado!";
      setTimeout(() => (btn.textContent = "\uD83D\uDCCB Copiar"), 2000);
    },
    () => {
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
    }
  );
}

async function exportReport() {
  const text = outputContent.textContent || outputContent.innerText;
  if (!text.trim()) return;

  const date = new Date().toISOString().slice(0, 19).replace(/[T:-]/g, "");
  const defaultName = "relatorio_rede_" + date + ".txt";

  try {
    const path = await save({
      defaultPath: defaultName,
      filters: [{ name: "Texto", extensions: ["txt"] }],
    });
    if (!path) return;
    await writeTextFile(path, text);
    appendOutput("\nRelatório exportado: " + path);
  } catch (err) {
    appendOutput("\n[ERRO] Falha ao exportar: " + err);
  }
}

async function exportHtml() {
  const text = outputContent.textContent || outputContent.innerText;
  if (!text.trim()) return;

  const date = new Date().toISOString().slice(0, 19).replace(/[T:-]/g, "");
  const defaultName = "relatorio_rede_" + date + ".html";

  const htmlContent = `<!DOCTYPE html>
<html lang="pt-BR">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>Relatório de Rede - ${date}</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:"Segoe UI",sans-serif;background:#1a1b26;color:#c0caf5;padding:40px;line-height:1.7}
h1{color:#7aa2f7;font-size:1.5rem;margin-bottom:8px}
pre{font-family:"Cascadia Code","JetBrains Mono",monospace;font-size:0.85rem;white-space:pre-wrap;word-break:break-all;background:#24253a;padding:24px;border-radius:8px;border:1px solid #3b4261;margin-top:16px}
.header{color:#565f89;font-size:0.9rem;margin-bottom:24px}
.problem{color:#f7768e;font-weight:700}
hr{border:none;border-top:1px solid #3b4261;margin:16px 0}
@media print{body{background:#fff;color:#000}pre{background:#f5f5f5;border-color:#ccc}}
</style>
</head>
<body>
<h1>King Analiser — Relatório de Rede</h1>
<div class="header">Gerado em ${new Date().toLocaleString("pt-BR")}</div>
<hr>
<pre>${text.replace(/</g, "&lt;").replace(/>/g, "&gt;")}</pre>
<hr>
<div class="header">Relatório gerado pelo King Analiser</div>
</body>
</html>`;

  try {
    const path = await save({
      defaultPath: defaultName,
      filters: [{ name: "HTML", extensions: ["html"] }],
    });
    if (!path) return;
    await writeTextFile(path, htmlContent);
    appendOutput("\nRelatório HTML exportado: " + path);
  } catch (err) {
    appendOutput("\n[ERRO] Falha ao exportar HTML: " + err);
  }
}

function saveToHistory(reportText) {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    const history = raw ? JSON.parse(raw) : [];
    const summary = reportText.split("\n").slice(0, 5).join(" ").substring(0, 120);
    history.unshift({
      timestamp: new Date().toISOString(),
      summary: summary + (summary.length >= 120 ? "..." : ""),
      reportText: reportText,
    });
    if (history.length > 10) history.length = 10;
    localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
  } catch (e) {
    console.error("Erro ao salvar histórico:", e);
  }
}

function loadHistory() {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function renderHistory() {
  const history = loadHistory();
  historyList.innerHTML = "";
  if (history.length === 0) {
    historyList.innerHTML = '<div class="history-item" style="color:var(--text-dim)">Nenhum relatório salvo no histórico.</div>';
    return;
  }
  for (const item of history) {
    const div = document.createElement("div");
    div.className = "history-item";
    const d = new Date(item.timestamp);
    const dateStr = d.toLocaleString("pt-BR");
    div.innerHTML = '<div class="history-date">' + dateStr + '</div><div class="history-summary">' + item.summary + "</div>";
    div.addEventListener("click", () => {
      outputContent.textContent = item.reportText;
      outputContent.scrollTop = 0;
      showOutput();
    });
    historyList.appendChild(div);
  }
}

function clearHistory() {
  localStorage.removeItem(HISTORY_KEY);
  renderHistory();
}

function toggleHistory() {
  if (historySection.classList.contains("hidden")) {
    renderHistory();
    showHistory();
  } else {
    historySection.classList.add("hidden");
  }
}

function toggleTheme() {
  const body = document.body;
  const btn = document.getElementById("btn-theme");
  const isLight = body.classList.toggle("light");
  btn.innerHTML = isLight ? "\u2600\uFE0F" : "\uD83C\uDF19";
  localStorage.setItem("kinganaliser_theme", isLight ? "light" : "dark");
}

function applyTheme() {
  const saved = localStorage.getItem("kinganaliser_theme");
  if (saved === "light") {
    document.body.classList.add("light");
    document.getElementById("btn-theme").innerHTML = "\u2600\uFE0F";
  }
}

document.addEventListener("DOMContentLoaded", () => {
  applyTheme();
  loadInterfaces();
  loadPortList();

  document.getElementById("btn-local-ip").addEventListener("click", showLocalIp);
  document.getElementById("btn-public-ip").addEventListener("click", showPublicIp);
  document.getElementById("btn-public-ip-info").addEventListener("click", showPublicIpInfo);
  document.getElementById("btn-dns").addEventListener("click", showDnsLookup);
  document.getElementById("btn-dns-bench").addEventListener("click", showDnsBench);
  document.getElementById("btn-ping").addEventListener("click", showPing);
  document.getElementById("btn-traceroute").addEventListener("click", showTraceroute);
  document.getElementById("btn-mtr").addEventListener("click", showMtr);
  document.getElementById("btn-gateway").addEventListener("click", showGateway);
  document.getElementById("btn-http").addEventListener("click", showHttpTiming);
  document.getElementById("btn-listening").addEventListener("click", showListeningPorts);
  document.getElementById("btn-scan").addEventListener("click", showPortScan);
  document.getElementById("btn-scan-network").addEventListener("click", showNetworkScan);
  document.getElementById("btn-iface-stats").addEventListener("click", showIfaceStats);
  document.getElementById("btn-report").addEventListener("click", showFullReport);
  document.getElementById("btn-copy").addEventListener("click", copyOutput);
  document.getElementById("btn-export").addEventListener("click", exportReport);
  document.getElementById("btn-export-html").addEventListener("click", exportHtml);
  document.getElementById("btn-clear").addEventListener("click", clearOutput);
  document.getElementById("btn-continuous-ping").addEventListener("click", doContinuousPing);
  document.getElementById("btn-stop-ping").addEventListener("click", stopContinuousPing);
  document.getElementById("btn-history").addEventListener("click", toggleHistory);
  document.getElementById("btn-clear-history").addEventListener("click", clearHistory);
  document.getElementById("btn-close-history").addEventListener("click", () => historySection.classList.add("hidden"));
  document.getElementById("btn-theme").addEventListener("click", toggleTheme);

  hostInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") showPing();
  });
  dnsInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") showDnsLookup();
  });

  hostInput.addEventListener("input", () => { hostError.textContent = ""; });
  dnsInput.addEventListener("input", () => { dnsError.textContent = ""; });

  ifaceSelect.addEventListener("change", () => {
    if (ifaceSelect.value) {
      hostInput.value = ifaceSelect.value;
    }
  });

  if (scanTimeoutSlider) {
    scanTimeoutSlider.addEventListener("input", () => {
      scanTimeoutLabel.textContent = scanTimeoutSlider.value + "ms";
    });
  }
});
