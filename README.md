# 🌐 KingNetworkTools — Analisador de Rede

KingNetworkTools é um aplicativo desktop para diagnóstico de rede feito com **Tauri v2**, **Rust** e **Vite**. Ele reúne ferramentas de conectividade em uma interface única: status da conexão, ping, traceroute, MTR, DNS, portas, scan de rede, timing HTTP e relatórios exportáveis.

O backend roda em Rust e executa as coletas diretamente no sistema operacional. O frontend é HTML, CSS e JavaScript vanilla.

## Destaques

- Dashboard com cartões de status para Gateway, DNS, Internet e Estabilidade.
- Relatório completo acessível pelo Dashboard e pela aba Relatórios.
- Relatórios TXT/HTML com tabelas ASCII, resumo executivo e detalhes úteis de cada diagnóstico.
- Ping contínuo com gráfico temporário de latência em tempo real.
- Traceroute com múltiplas tentativas no Linux: padrão, TCP/443, ICMP e `tracepath`.
- MTR integrado na aba Traceroute, para evitar duplicidade de navegação.
- Scan de rede `/24` com concorrência controlada para reduzir o tempo de varredura.
- Ícone customizado gerado com `tauri icon`.

## Funcionalidades

| Área | O que faz |
|---|---|
| Dashboard | Mostra IP local, IP público, gateway, DNS, latência e status atual da conexão |
| Ping | Mede pacotes enviados/recebidos, perda, mínimo, média, máximo e jitter |
| Ping contínuo | Emite amostras em tempo real e exibe gráfico de latência |
| Traceroute | Mapeia saltos até o destino e classifica hops por latência/perda |
| MTR | Mede perda, média, melhor/pior latência e jitter por hop, dentro da aba Traceroute |
| IP / Gateway | Lista interfaces, IP público, geolocalização e gateways padrão |
| DNS | Faz lookup do alvo e benchmark de resolvedores conhecidos |
| Portas | Lista portas locais em escuta e faz scan TCP em portas comuns |
| Scan de Rede | Varre uma sub-rede `/24` com concorrência limitada |
| HTTP Timing | Mede tempo total de acesso a alvos HTTP predefinidos |
| Relatórios | Consolida os resultados em relatório detalhado, com histórico local e exportação |

## Traceroute vs MTR

**Traceroute** executa uma rota pontual até o destino e mostra os saltos encontrados. É bom para entender o caminho.

**MTR** combina traceroute com medições repetidas por hop. É melhor para observar perda, jitter e instabilidade ao longo do caminho.

No app, os dois ficam na mesma aba porque investigam o mesmo problema por perspectivas complementares.

## Stack

- Frontend: HTML, CSS e JavaScript vanilla
- Bundler: Vite 6
- Desktop: Tauri v2
- Backend: Rust
- IPC: `@tauri-apps/api/core`
- Plugins: `@tauri-apps/plugin-dialog` e `@tauri-apps/plugin-fs`
- HTTP: `ureq 3.x`
- Sistema: `ping`, `traceroute`, `tracepath`, `mtr`, `ss`, `netstat`, `ip`, `arp`, PowerShell no Windows

## Requisitos

- Node.js 18+
- Rust stable
- Tauri CLI v2

```bash
cargo install tauri-cli --version "^2"
```

Dependências Linux comuns:

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev \
  libappindicator3-dev librsvg2-dev patchelf
```

Ferramentas recomendadas no Linux:

```bash
sudo apt-get install -y iproute2 iputils-ping traceroute iputils-tracepath mtr-tiny dnsutils
```

## Desenvolvimento

```bash
npm install
npm run tauri dev
```

Para testar apenas o frontend:

```bash
npm run dev
```

Os comandos IPC de rede dependem do backend Tauri, então não funcionam no navegador puro.

## Build

```bash
npm run tauri build
```

Os artefatos ficam em:

```text
src-tauri/target/release/bundle/
```

## Testes

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

## Ícones

Os ícones ficam em `src-tauri/icons/`. Para regerar:

```bash
tauri icon /caminho/para/icon.png
```

## Estrutura

```text
.
├── index.html
├── style.css
├── vite.config.js
├── src/
│   └── main.js
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/default.json
    └── src/
        ├── main.rs
        ├── lib.rs
        ├── commands.rs
        └── analyzer/
            ├── dns.rs
            ├── dns_bench.rs
            ├── gateway.rs
            ├── http_timing.rs
            ├── iface_stats.rs
            ├── ip.rs
            ├── mtr.rs
            ├── network_scan.rs
            ├── ports.rs
            ├── quality.rs
            ├── report.rs
            ├── route.rs
            └── tests.rs
```

## Comportamento por plataforma

| Função | Linux | Windows |
|---|---|---|
| Ping | `ping -c` | `ping -n` |
| Traceroute | `traceroute`, `traceroute -T`, `traceroute -I`, fallback `tracepath` | `tracert` |
| MTR | `mtr --report` com fallback para traceroute | fallback baseado em `tracert` + `ping` |
| Portas locais | `ss -tln4` | `netstat -ano` |
| Gateway | `ip route show` | `netstat -rn` |
| Interfaces | `ip -j addr show` | PowerShell `Get-NetAdapter` |

## Limitações conhecidas

- Scan de rede atualmente aceita apenas sub-redes `/24`.
- Alguns ambientes bloqueiam ICMP/UDP/TCP usado por traceroute/MTR; nesses casos o app mostra aviso de rota parcialmente filtrada.
- `mtr`, `traceroute`, `tracepath`, `dig` e `nslookup` podem não estar instalados por padrão.
- IP público e geolocalização dependem de conectividade com serviços externos.
- Hosts que bloqueiam ICMP podem aparecer com perda alta mesmo quando serviços TCP funcionam.

## Licença

MIT
