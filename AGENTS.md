# Analisador de Rede — Instruções para Agentes

## Stack

- **Frontend:** HTML + CSS + JS vanilla, bundlizado com Vite 6
- **Backend:** Rust, Tauri v2, IPC via `@tauri-apps/api/core`
- **Ícones:** gerar com `tauri icon <source.png>` antes do build de produção

## Comandos

```bash
npm install                        # instalar frontend (Vite + @tauri-apps/api)
cargo install tauri-cli --version "^2"  # CLI do Tauri v2
npm run tauri dev                  # iniciar em modo dev (Vite + Tauri)
npm run tauri build                # build de produção
```

Se o `tauri` CLI não estiver no PATH, use `npx tauri dev` / `npx tauri build`.

## Estrutura

```
analisador-de-rede/
├── index.html                     # entrada do frontend
├── vite.config.js                 # config Vite (porta 1420, HMR)
├── src/main.js                    # lógica frontend (invoke dos comandos)
├── style.css                      # tema escuro
├── src-tauri/
│   ├── Cargo.toml                 # deps Rust: tauri 2, serde, ureq, local-ip-address
│   ├── tauri.conf.json            # config Tauri v2 (janela, build, bundle)
│   ├── capabilities/default.json  # permissões (core:default)
│   └── src/
│       ├── main.rs                # entry point desktop (só chama lib::run)
│       ├── lib.rs                 # registra os comandos Tauri
│       ├── commands.rs            # comandos Tauri (IPC)
│       └── analyzer/
│           ├── ip.rs              # IP local (local-ip-address) + público (ureq)
│           ├── route.rs           # ping + traceroute via processo externo
│           ├── ports.rs           # portas escuta (netstat/ss) + scan TCP
│           └── report.rs          # geração de relatório agregado
```

## Comandos Tauri disponíveis (IPC)

| Comando | Retorno | Descrição |
|---|---|---|
| `get_local_ip` | `Vec<InterfaceInfo>` | IP local + gateway |
| `get_public_ip` | `String` | IP público via api.ipify.org |
| `ping` | `PingResult` | Ping com 4 pacotes |
| `trace_route` | `Vec<Hop>` | Traceroute até 30 hops |
| `get_listening_ports` | `Vec<ListeningPort>` | Portas em escuta (netstat/ss) |
| `scan_ports` | `Vec<ScanResult>` | TCP connect scan em portas alvo |
| `generate_report` | `String` | Relatório completo em texto |

## Detalhes importantes

### Cross-platform
- `ping`: Linux usa `-c`, Windows usa `-n`
- `traceroute`: Linux usa `traceroute -n -m 30`, Windows usa `tracert -h 30 -d`
- **Linux:** `traceroute` pode não vir instalado (`apt install inetutils-traceroute` ou `apt install traceroute`)
- Portas escuta: Linux `ss -tlnp4`, Windows `netstat -ano`

### Scan de portas
- TCP connect scan via `std::net::TcpStream::connect_timeout` (timeout 1500ms)
- Portas comuns pré-definidas (29 portas: 21,22,23,25,53,80,110,111,135,139,143,443,445,993,995,1433,1521,2049,3306,3389,5432,5900,5985,5986,6379,8080,8443,9090,27017)
- Pode passar lista customizada via `ports_list` no `scan_ports`

### Gateway
- Gateway padrão extraído de `ip route show default` (Linux) ou `netstat -rn` (Windows)

### Permissões
- Apenas `core:default` no `capabilities/default.json` — comandos shell executam no backend Rust fora do sistema de permissões Tauri

### Limitações conhecidas
- `traceroute` requer permissão root ou `sudo` em alguns sistemas — o app tentará executar sem privilégios
- `IP público` depende de conectividade com api.ipify.org (porta 443)

## Fluxo de trabalho típico

1. `npm install`
2. `npm run tauri dev` (inicia Vite + Tauri simultaneamente)
3. Para testar apenas o frontend: `npm run dev` (abre http://localhost:1420 — comandos não funcionarão sem Tauri)
4. Para build: `npm run tauri build`

## Requisitos

- **Node.js** 18+ (para Vite e frontend)
- **Rust** (instalar via `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Tauri CLI v2:** `cargo install tauri-cli --version "^2"`
- **Linux:** `libwebkit2gtk-4.1-dev`, `libgtk-3-dev` e outras — veja https://v2.tauri.app/start/prerequisites/

## Correções aplicadas durante o desenvolvimento

- `commands.rs`: funções precisam ser `pub fn` para o `generate_handler!()` no Tauri v2
- `ip.rs`: `ureq 3.x` usa `ureq::Config::builder().timeout_global(Some(Duration)).build()` e `Agent::new_with_config(config)`. Leitura do body via `resp.into_body().read_to_string()`
- `route.rs`: `MAX_HOPS.to_string()` precisa ser vinculado a uma variável (`let max_hops_str = ...`) antes de passar referência para `vec![]` (lifetime)
- `ports.rs`: no `ss -tln4` (Linux), coluna do endereço:porta é `parts[4]` (não `parts[3]`)
- `ports.rs`: `ss -tln4` sem `-p` (não requer root)
- Ícones: Tauri v2 exige PNG RGBA (color type 6), não RGB
- `package.json` precisa do script `"tauri": "tauri"` para `npm run tauri dev` funcionar
- `tauri` CLI instalado via `cargo install tauri-cli --version "^2"`, binário como `cargo-tauri` — criar symlink `ln -sf ~/.cargo/bin/cargo-tauri ~/.cargo/bin/tauri`
