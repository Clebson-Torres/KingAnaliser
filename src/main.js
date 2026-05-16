import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

const hostInput = document.getElementById("host-input");
const dnsInput = document.getElementById("dns-input");
const ifaceSelect = document.getElementById("iface-select");
const outputContent = document.getElementById("output-content");
const outputSection = document.getElementById("output");
const statusBar = document.getElementById("status-bar");
const statusText = document.getElementById("status-text");

let portList = [];

function showStatus(msg) {
  statusBar.classList.remove("hidden");
  statusText.textContent = msg;
}

function hideStatus() {
  statusBar.classList.add("hidden");
}

function showOutput() {
  outputSection.classList.remove("hidden");
}

function clearOutput() {
  outputContent.textContent = "";
  outputSection.classList.add("hidden");
}

function appendOutput(text) {
  outputContent.textContent += text + "\n";
  outputContent.scrollTop = outputContent.scrollHeight;
}

function appendSection(title) {
  appendOutput("");
  const line = "\u2500".repeat(Math.min(50, title.length + 10));
  appendOutput(line);
  appendOutput("  " + title);
  appendOutput(line);
}

async function withLoading(msg, fn) {
  showStatus(msg);
  showOutput();
  try {
    const result = await fn();
    return result;
  } finally {
    hideStatus();
  }
}

function catchError(err) {
  appendOutput("[ERRO] " + err);
  console.error(err);
}

async function loadInterfaces() {
  try {
    const interfaces = await invoke("get_network_interfaces");
    ifaceSelect.innerHTML = "";
    for (const iface of interfaces) {
      const opt = document.createElement("option");
      opt.value = iface.ip;
      opt.textContent = iface.name + " (" + iface.ip + ")" + (iface.mac ? " - " + iface.mac : "") + (iface.is_up ? " \u2713" : " \u2717");
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
    portList = [21, 22, 23, 25, 53, 80, 110, 111, 135, 139, 143, 443, 445, 993, 995, 1433, 1521, 2049, 3306, 3389, 5432, 5900, 5985, 5986, 6379, 8080, 8443, 9090, 27017];
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
  await withLoading("Obtendo IP local...", async () => {
    appendSection("Interfaces de Rede");
    const interfaces = await invoke("get_network_interfaces");
    const rows = interfaces.map(i => [i.name, i.ip, i.mac || "-", i.is_up ? "Ativa" : "Inativa"]);
    appendOutput(toTable(["Interface", "IP", "MAC", "Status"], rows));
  }).catch(catchError);
}

async function showPublicIp() {
  await withLoading("Consultando IP p\u00fablico...", async () => {
    appendSection("IP P\u00fablico");
    const ip = await invoke("get_public_ip");
    appendOutput("  IP P\u00fablico: " + ip);
  }).catch(catchError);
}

async function showDnsLookup() {
  const host = dnsInput.value.trim();
  if (!host) return;
  await withLoading("Resolvendo DNS para " + host + "...", async () => {
    appendSection("DNS Lookup: " + host);
    const result = await invoke("dns_lookup", { host });
    appendOutput("  Host:     " + result.host);
    appendOutput("  IPs:");
    for (const ip of result.addresses) {
      appendOutput("    - " + ip);
    }
    appendOutput("  Reverso:  " + (result.reverse || "(n\u00e3o dispon\u00edvel)"));
  }).catch(catchError);
}

async function showDnsBench() {
  await withLoading("Executando benchmark DNS...", async () => {
    appendSection("DNS Benchmark");
    appendOutput("  Testando servidores DNS... (pode levar alguns segundos)\n");
    const results = await invoke("benchmark_dns");
    if (results.length === 0) {
      appendOutput("  Nenhum resultado obtido.");
      return;
    }
    const rows = results.map(r => [r.name, r.ip, r.latency_ms + "ms", r.status]);
    appendOutput(toTable(["Servidor", "IP", "Lat\u00eancia", "Status"], rows));
    if (results[0].status === "OK") {
      appendOutput("\n  Mais r\u00e1pido: " + results[0].name + " (" + results[0].latency_ms + "ms)");
    }
  }).catch(catchError);
}

async function showPing() {
  const host = hostInput.value.trim();
  if (!host) return;
  await withLoading("Executando ping para " + host + "...", async () => {
    appendSection("Ping: " + host);
    const result = await invoke("ping", { host });
    appendOutput("  Host:        " + result.host);
    appendOutput("  Transmitido: " + result.transmitted);
    appendOutput("  Recebido:    " + result.received);
    appendOutput("  Perda:       " + result.loss_pct + "%");
    if (result.received > 0) {
      appendOutput("  M\u00ednimo:      " + result.min_ms.toFixed(1) + " ms");
      appendOutput("  M\u00e9dio:       " + result.avg_ms.toFixed(1) + " ms");
      appendOutput("  M\u00e1ximo:      " + result.max_ms.toFixed(1) + " ms");
    }
  }).catch(catchError);
}

async function showTraceroute() {
  const host = hostInput.value.trim();
  if (!host) return;
  await withLoading("Tra\u00e7ando rota para " + host + "...", async () => {
    appendSection("Rota at\u00e9 " + host);
    const hops = await invoke("trace_route", { host });
    for (const hop of hops) {
      const num = String(hop.hop_number).padStart(2, " ");
      const addr = hop.address.padEnd(22);
      appendOutput("  " + num + ". " + addr + " (" + hop.latency_ms + ")");
    }
  }).catch(catchError);
}

async function showGateway() {
  await withLoading("Obtendo informa\u00e7\u00f5es do gateway...", async () => {
    appendSection("Gateway");
    const info = await invoke("get_gateway_info");
    appendOutput("  IP:       " + info.ip);
    appendOutput("  Interface: " + info.interface);
    appendOutput("  Lat\u00eancia:  " + (info.latency_ms !== null ? info.latency_ms.toFixed(1) + " ms" : "N/A"));
    appendOutput("  Qualidade: " + info.quality);
  }).catch(catchError);
}

async function showHttpTiming() {
  await withLoading("Testando timing HTTP...", async () => {
    appendSection("HTTP Timing");
    const targets = await invoke("get_http_targets");
    for (const url of targets) {
      appendOutput("  Testando: " + url + "...");
      try {
        const t = await invoke("test_http_timing", { url });
        appendOutput(toTable(
          ["DNS", "Connect", "TTFB", "Total", "HTTP"],
          [[t.dns_s.toFixed(3) + "s", t.connect_s.toFixed(3) + "s", t.ttfb_s.toFixed(3) + "s", t.total_s.toFixed(3) + "s", t.status_code]]
        ));
      } catch (e) {
        appendOutput("    [ERRO] " + e + "\n");
      }
    }
  }).catch(catchError);
}

async function showListeningPorts() {
  await withLoading("Obtendo portas em escuta...", async () => {
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
  const host = hostInput.value.trim() || "127.0.0.1";
  await withLoading("Escaneando portas em " + host + "...", async () => {
    appendSection("Scan de Portas: " + host);
    const results = await invoke("scan_ports", { host, portsList: portList });
    const open = results.filter(r => r.state === "ABERTA");
    if (open.length === 0) {
      appendOutput("  Nenhuma porta aberta encontrada.");
    } else {
      const rows = open.map(r => [String(r.port), r.service, r.state, r.latency_ms + "ms"]);
      appendOutput(toTable(["Porta", "Servi\u00e7o", "Estado", "Lat\u00eancia"], rows));
    }
    appendOutput("\n  Escaneadas: " + results.length + ", Abertas: " + open.length);
  }).catch(catchError);
}

async function showIfaceStats() {
  await withLoading("Obtendo estat\u00edsticas de interface...", async () => {
    appendSection("Estat\u00edsticas de Interface");
    const stats = await invoke("get_interface_stats");
    if (stats.length === 0) {
      appendOutput("  Nenhuma estat\u00edstica dispon\u00edvel.");
      return;
    }
    const rows = stats.map(s => [s.name, s.rx_mb.toFixed(2) + " MB", s.tx_mb.toFixed(2) + " MB", String(s.rx_errors), String(s.tx_errors), String(s.rx_dropped)]);
    appendOutput(toTable(["Interface", "Recebido", "Enviado", "Err RX", "Err TX", "Drop RX"], rows));
  }).catch(catchError);
}

async function showFullReport() {
  await withLoading("Gerando relat\u00f3rio completo...", async () => {
    const host = hostInput.value.trim() || "8.8.8.8";

    appendSection("Coletando dados...");

    let ipLocalText = "", ipPubText = "", dnsText = "", pingText = "",
        tracerouteText = "", portsText = "", scanText = "",
        gatewayText = "", dnsBenchText = "", httpText = "", ifaceStatsText = "";

    try {
      const ifaces = await invoke("get_network_interfaces");
      ipLocalText = ifaces.map(i => "  " + i.name + ": " + i.ip + " (" + (i.is_up ? "ativa" : "inativa") + ")").join("\n");
    } catch (e) { ipLocalText = "  [ERRO] " + e; }

    try {
      ipPubText = "  " + (await invoke("get_public_ip"));
    } catch (e) { ipPubText = "  [ERRO] " + e; }

    try {
      const d = await invoke("dns_lookup", { host });
      dnsText = "  " + d.host + " -> " + d.addresses.join(", ");
    } catch (e) { dnsText = "  [ERRO] " + e; }

    try {
      const p = await invoke("ping", { host });
      pingText = "  Perda: " + p.loss_pct + "%" + (p.received > 0 ? ", M\u00e9dia: " + p.avg_ms.toFixed(1) + "ms" : "");
    } catch (e) { pingText = "  [ERRO] " + e; }

    try {
      const h = await invoke("trace_route", { host });
      tracerouteText = "  " + h.length + " hops (at\u00e9 " + host + ")";
    } catch (e) { tracerouteText = "  [ERRO] " + e; }

    try {
      const p = await invoke("get_listening_ports");
      portsText = "  " + p.length + " portas em escuta";
    } catch (e) { portsText = "  [ERRO] " + e; }

    try {
      const r = await invoke("scan_ports", { host, portsList: portList });
      const open = r.filter(x => x.state === "ABERTA");
      scanText = "  " + r.length + " escaneadas, " + open.length + " abertas";
    } catch (e) { scanText = "  [ERRO] " + e; }

    try {
      const g = await invoke("get_gateway_info");
      gatewayText = "  " + g.ip + " (" + g.interface + ") - " + g.quality;
    } catch (e) { gatewayText = "  [ERRO] " + e; }

    try {
      const b = await invoke("benchmark_dns");
      dnsBenchText = "  " + b.length + " servidores testados" + (b[0] ? ", melhor: " + b[0].name + " (" + b[0].latency_ms + "ms)" : "");
    } catch (e) { dnsBenchText = "  [ERRO] " + e; }

    try {
      const targets = await invoke("get_http_targets");
      const timings = [];
      for (const u of targets) {
        try {
          const t = await invoke("test_http_timing", { url: u });
          timings.push(t.total_s.toFixed(2) + "s");
        } catch { timings.push("ERRO"); }
      }
      httpText = "  " + targets.map((u, i) => u + ": " + timings[i]).join("\n  ");
    } catch (e) { httpText = "  [ERRO] " + e; }

    try {
      const s = await invoke("get_interface_stats");
      ifaceStatsText = "  " + s.map(x => x.name + ": RX " + x.rx_mb.toFixed(1) + "MB, TX " + x.tx_mb.toFixed(1) + "MB").join("\n  ");
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
  }).catch(catchError);
}

function copyOutput() {
  const text = outputContent.textContent;
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
  const text = outputContent.textContent;
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
    appendOutput("\nRelat\u00f3rio exportado: " + path);
  } catch (err) {
    appendOutput("\n[ERRO] Falha ao exportar: " + err);
  }
}

document.addEventListener("DOMContentLoaded", () => {
  loadInterfaces();
  loadPortList();

  document.getElementById("btn-local-ip").addEventListener("click", showLocalIp);
  document.getElementById("btn-public-ip").addEventListener("click", showPublicIp);
  document.getElementById("btn-dns").addEventListener("click", showDnsLookup);
  document.getElementById("btn-dns-bench").addEventListener("click", showDnsBench);
  document.getElementById("btn-ping").addEventListener("click", showPing);
  document.getElementById("btn-traceroute").addEventListener("click", showTraceroute);
  document.getElementById("btn-gateway").addEventListener("click", showGateway);
  document.getElementById("btn-http").addEventListener("click", showHttpTiming);
  document.getElementById("btn-listening").addEventListener("click", showListeningPorts);
  document.getElementById("btn-scan").addEventListener("click", showPortScan);
  document.getElementById("btn-iface-stats").addEventListener("click", showIfaceStats);
  document.getElementById("btn-report").addEventListener("click", showFullReport);
  document.getElementById("btn-copy").addEventListener("click", copyOutput);
  document.getElementById("btn-export").addEventListener("click", exportReport);
  document.getElementById("btn-clear").addEventListener("click", clearOutput);

  hostInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") showPing();
  });
  dnsInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") showDnsLookup();
  });

  ifaceSelect.addEventListener("change", () => {
    if (ifaceSelect.value) {
      hostInput.value = ifaceSelect.value;
    }
  });
});
