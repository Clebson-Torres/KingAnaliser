# 🌐 KingAnaliser — Analisador de Rede

App desktop para diagnóstico completo de rede, construído com **Tauri v2 + Rust + Vite**.
Executa análises diretamente no sistema operacional via backend Rust, sem dependência de serviços externos além do IP público.

![Build](https://github.com/Clebson-Torres/KingAnaliser/actions/workflows/build.yml/badge.svg)
![Rust](https://img.shields.io/badge/Rust-64.9%25-orange?logo=rust)
![Tauri](https://img.shields.io/badge/Tauri-v2-blue?logo=tauri)
![Plataformas](https://img.shields.io/badge/Linux%20%7C%20Windows-suportados-brightgreen)

---

## ✨ Funcionalidades

| Comando | Descrição |
|---|---|
| **IP Local** | Detecta IP da interface ativa e gateway padrão |
| **IP Público** | Consulta IP externo via `api.ipify.org` |
| **Ping** | Envia 4 pacotes ICMP e exibe latência + perda |
| **Traceroute** | Mapeia todos os saltos até o destino (até 30 hops) |
| **Portas em Escuta** | Lista portas TCP abertas localmente via `ss`/`netstat` |
| **Scan TCP** | Varre 29 portas comuns via `TcpStream::connect_timeout` |
| **Relatório** | Consolida todos os resultados em texto exportável |

### Extras
- **Exportar** — diálogo nativo "Salvar como" para `.txt`
- **Copiar** — resultado direto para a área de transferência
- **Interface assíncrona** — todos os comandos rodam em thread pool, sem travar a UI
- **Tema Tokyo Night** — interface escura de alto contraste

---

## 📦 Download

Acesse a página de [Releases](https://github.com/Clebson-Torres/KingAnaliser/releases) e baixe o instalador:

| Plataforma | Arquivo |
|---|---|
| Windows | `KingAnaliser_x.x.x_x64-setup.exe` |
| Windows (MSI) | `KingAnaliser_x.x.x_x64_en-US.msi` |
| Linux (AppImage) | `king-analiser_x.x.x_amd64.AppImage` |
| Linux (Debian/Ubuntu) | `king-analiser_x.x.x_amd64.deb` |

---

## 🛠️ Stack

- **Frontend:** HTML + CSS + JavaScript vanilla — [Vite 6](https://vitejs.dev/)
- **Backend:** [Rust](https://www.rust-lang.org/) + [Tauri v2](https://v2.tauri.app/)
- **IPC:** `@tauri-apps/api/core` → `invoke()`
- **HTTP:** `ureq 3.x` (IP público)
- **Rede local:** `local-ip-address` crate

---

## 🚀 Desenvolvimento local

### Pré-requisitos

- [Node.js 18+](https://nodejs.org/)
- [Rust stable](https://rustup.rs/)
- Tauri CLI v2:
```bash
cargo install tauri-cli --version "^2"
```

**Dependências Linux (Debian/Ubuntu):**
```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev \
  libappindicator3-dev librsvg2-dev patchelf
```

**Dependências Linux (Fedora):**
```bash
sudo dnf install webkit2gtk4.1-devel gtk3-devel \
  libappindicator-gtk3-devel librsvg2-devel
```

### Rodando em modo dev

```bash
npm install
npm run tauri dev
```

> Para testar só o frontend (sem Tauri): `npm run dev` — os comandos IPC não funcionam sem o backend.

### Build local

```bash
npm run tauri build
```

Artefatos gerados em `src-tauri/target/release/bundle/`.

---

## 🗂️ Estrutura do projeto

```
KingAnaliser/
├── index.html                      # UI principal
├── style.css                       # Tema Tokyo Night
├── vite.config.js                  # Porta 1420, HMR
├── src/
│   └── main.js                     # Lógica frontend + invoke()
└── src-tauri/
    ├── Cargo.toml                   # Rust: tauri 2, serde, ureq, local-ip-address
    ├── tauri.conf.json              # Janela, build, bundle
    ├── capabilities/
    │   └── default.json             # Permissões Tauri (core:default)
    └── src/
        ├── lib.rs                   # Registra comandos
        ├── commands.rs              # Handlers IPC
        └── analyzer/
            ├── ip.rs                # IP local + público
            ├── route.rs             # Ping + traceroute
            ├── ports.rs             # Portas em escuta + scan TCP
            └── report.rs            # Geração de relatório
```

---

## ⚙️ Comportamento cross-platform

O backend adapta os comandos automaticamente ao sistema operacional:

| Função | Linux | Windows |
|---|---|---|
| Ping | `ping -c 4` | `ping -n 4` |
| Traceroute | `traceroute -n -m 30` | `tracert -h 30 -d` |
| Portas | `ss -tlnp4` | `netstat -ano` |
| Gateway | `ip route show default` | `netstat -rn` |

---

## 🔄 CI/CD — Build automático

O projeto usa GitHub Actions para gerar os instaladores automaticamente ao criar uma tag:

```bash
git tag v1.0.0
git push origin v1.0.0
```

Os builds de Linux e Windows rodam em paralelo e são publicados automaticamente como Release.

---

## ⚠️ Limitações conhecidas

- `traceroute` pode exigir `sudo` em alguns sistemas Linux — o app tenta sem privilégios
- IP público depende de conectividade com `api.ipify.org` (porta 443)
- Ping reporta 100% de perda em hosts que bloqueiam ICMP (comportamento esperado)

---

## 📄 Licença

MIT — veja [LICENSE](LICENSE) para detalhes.
