# <img src="assets/icon.png" alt="Docker UI" width="32" height="32"> Docker UI

Uma aplicação de monitoramento Docker construída com Rust e Slint, oferecendo uma interface gráfica moderna para visualizar estatísticas e gerenciar containers.

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white"/>
  <img alt="Docker" src="https://img.shields.io/badge/Docker-2496ED?style=for-the-badge&logo=docker&logoColor=white"/>
  <img alt="GUI" src="https://img.shields.io/badge/GUI-Slint-blue?style=for-the-badge"/>
  <img alt="Real-time" src="https://img.shields.io/badge/Real--time-Monitoring-green?style=for-the-badge"/>
</p>

## 📸 Screenshots

<p align="center">
  <img src="images/dashboard.png" alt="Dashboard Principal" width="600"/>
  <br>
  <em>Dashboard principal com estatísticas em tempo real</em>
</p>

### 🖼️ Galeria Completa de Interface

<table>
  <tr>
    <td align="center">
      <img src="images/dashboard.png" width="280"/>
      <br><strong>📊 Dashboard</strong>
      <br><em>Monitoramento em tempo real</em>
    </td>
    <td align="center">
      <img src="images/containers.png" width="280"/>
      <br><strong>🐳 Containers</strong>
      <br><em>Gerenciamento completo</em>
    </td>
    <td align="center">
      <img src="images/images.png" width="280"/>
      <br><strong>🖼️ Images</strong>
      <br><em>Controle de imagens Docker</em>
    </td>
  </tr>
  <tr>
    <td align="center">
      <img src="images/networks.png" width="280"/>
      <br><strong>🌐 Networks</strong>
      <br><em>Redes personalizadas</em>
    </td>
    <td align="center">
      <img src="images/volumes.png" width="280"/>
      <br><strong>💾 Volumes</strong>
      <br><em>Armazenamento persistente</em>
    </td>
    <td align="center">
      <div style="height: 200px; display: flex; align-items: center; justify-content: center; background: #f8f9fa; border: 2px dashed #dee2e6; border-radius: 8px;">
        <span style="color: #6c757d; font-size: 14px;">Interface Completa</span>
      </div>
      <br><strong>✨ Funcionalidades</strong>
      <br><em>Todas as telas em uma UI</em>
    </td>
  </tr>
</table>

## ✨ Funcionalidades

### 📊 **Dashboard & Monitoramento**
- **Dashboard em tempo real** - CPU, memória e rede com gráficos interativos
- **Gráficos históricos** - Últimos 60 pontos de dados atualizados a cada segundo
- **Status do Docker** - Verificação automática do daemon e informações do sistema

### 🐳 **Gerenciamento de Containers**
- **Lista completa** - Todos os containers (rodando, parados, pausados)
- **Controles avançados** - Start, stop, pause, unpause, remove
- **Busca e filtros** - Por nome, status (all/running/stopped/paused)
- **Atualização em tempo real** - Lista atualizada automaticamente

### 🖼️ **Gerenciamento de Imagens**
- **Lista de imagens** - Com tags, tamanho e tempo de criação
- **Status de uso** - Indica se imagem está sendo usada por containers
- **Remoção segura** - Impede exclusão de imagens em uso
- **Ordenação consistente** - Lista mantém ordem alfabética

### 🌐 **Gerenciamento de Networks**
- **Networks personalizadas** - Exclui networks de sistema (bridge, host, none)
- **Contagem de containers** - Mostra quantos containers estão conectados
- **Proteção inteligente** - Impede remoção de networks em uso
- **Indicadores visuais** - Verde (disponível) / Amarelo (em uso)

### 💾 **Gerenciamento de Volumes**
- **Volumes ativos** - Mostra apenas volumes com containers conectados
- **Path completo** - Exibe mountpoint com truncagem inteligente
- **Proteção de dados** - Impede remoção de volumes em uso
- **Driver e metadata** - Informações detalhadas de cada volume

### 🌐 **Gerenciamento Remoto SSH**
- **Conexão SSH** - Conecte a servidores remotos via SSH
- **Docker remoto** - Gerencie containers Docker em servidores remotos
- **Toggle automático** - Alternância automática entre local e remoto
- **Log de containers** - Exibe containers do servidor no terminal ao conectar
- **Persistência de servidores** - Salva configurações de servidores SSH

### ⚡ **Funcionalidades Avançadas**
- **Interface modular** - Componentes separados e reutilizáveis
- **Mensagens temporárias** - Feedback com auto-dismiss em 3 segundos
- **Ordenação consistente** - Listas mantêm ordem entre atualizações
- **Performance otimizada** - Renderização eficiente com Slint
- **Arquitetura limpa** - Separação UI/lógica com padrões consistentes

## 🚀 Pré-requisitos

- **Rust** 1.70+ 
- **Docker** instalado e rodando
- **SSH client** (para conexões remotas)
- **Servidores SSH** com Docker instalado (para gerenciamento remoto)

### Dependências por sistema

#### Ubuntu/Debian:
```bash
sudo apt update
sudo apt install build-essential pkg-config libfontconfig1-dev
```

#### openSUSE:
```bash
sudo zypper refresh
sudo zypper install gcc gcc-c++ pkg-config fontconfig-devel rpm-build
```

#### Instalação automática:
```bash
make deps  # Detecta o sistema automaticamente
```

## 📦 Instalação

1. **Clone o repositório:**
   ```bash
   git clone <url-do-repositorio>
   cd teste-docker
   ```

2. **Compile e execute:**
   ```bash
   # Usando Cargo diretamente
   cargo run
   
   # Ou usando Makefile
   make dev
   ```

## 📦 Build e Distribuição

### Build rápido para desenvolvimento
```bash
make build          # Build release
make dev            # Run em modo desenvolvimento
make watch          # Run com auto-reload
```

### Geração de pacotes

#### Para sistemas Debian/Ubuntu (.deb)
```bash
# Gerar pacote .deb versionado
./build-deb.sh
# ou
make deb

# Build completo (check, test, build, package)
make release
```

#### Para openSUSE (.rpm)
```bash
# Gerar pacote .rpm versionado
./build-rpm.sh
# ou
make rpm

# Build completo para openSUSE (check, test, build, rpm)
make release-rpm
```

### Gerenciamento de builds
```bash
# Listar todos os builds
make list-builds

# Limpar builds antigos (manter 5 mais recentes)
make clean-builds

# Limpar todos os builds
make clean-all-builds
```

### Instalação local

#### Para sistemas Debian/Ubuntu
```bash
# Instalar pacote .deb localmente
make install

# Desinstalar
make uninstall

# Reinstalar
make reinstall
```

#### Para openSUSE
```bash
# Instalar pacote .rpm localmente
make install-rpm

# Desinstalar
make uninstall

# Reinstalar
make reinstall-rpm

# Instalação manual
sudo rpm -ivh builds/docker-ui-*.rpm

# Desinstalação manual
sudo rpm -e docker-ui
```

## 🛠️ Desenvolvimento

### Modo watch (recompilação automática)
```bash
# Instalar cargo-watch
cargo install cargo-watch

# Executar em modo watch
cargo watch -x run

# Watch com limpeza de tela
cargo watch -c -x run
```

### Desenvolvimento de UI
A interface utiliza **imports modulares** do Slint:
```slint
import { DashboardView } from "dashboard.slint";
import { ContainersList } from "containers.slint";
import { ImagesList } from "images.slint";
// ...
```

Cada componente é independente e reutilizável, facilitando manutenção e desenvolvimento.

### Estrutura do projeto
```
├── src/
│   ├── main.rs              # Aplicação principal e gerenciamento de estado
│   ├── docker.rs            # API Docker local e coleta de estatísticas
│   ├── docker_remote.rs     # API Docker remota via SSH
│   ├── docker_manager_switch.rs # Sistema de toggle local/remoto
│   ├── ssh.rs               # Cliente SSH e gerenciamento de conexões
│   ├── ssh_persistence.rs   # Persistência de configurações SSH
│   ├── ssh_ui_integration.rs # Integração SSH com UI
│   ├── chart.rs             # Renderização de gráficos
│   ├── lib.rs               # Módulos da biblioteca
│   └── build.rs             # Script de compilação Slint
├── ui/
│   ├── app.slint            # Interface principal e janela
│   ├── dashboard.slint      # Dashboard com estatísticas
│   ├── containers.slint     # Tela de containers
│   ├── container.slint      # Componentes individuais de container
│   ├── images.slint         # Tela de imagens Docker
│   ├── network.slint        # Tela de redes
│   ├── volumes.slint        # Tela de volumes
│   └── ssh_servers.slint    # Tela de gerenciamento SSH
├── assets/
│   └── *.png                # Ícones da aplicação (múltiplos tamanhos)
├── images/
│   └── *.png                # Screenshots da aplicação
├── builds/                  # Pacotes .deb gerados (criado automaticamente)
├── ssh_servers.json         # Configurações de servidores SSH (criado automaticamente)
├── build-deb.sh             # Script de build versionado
├── clean-builds.sh          # Script de limpeza de builds
├── Makefile                 # Sistema de build automatizado
└── Cargo.toml               # Dependências do projeto
```

## 🎯 Como usar

1. **Execute a aplicação:**
   ```bash
   cargo run
   ```

2. **Navegue pelas abas:**
   - **Docker UI**: Dashboard principal com estatísticas em tempo real
   - **Containers**: Gerenciamento completo (start/stop/pause/remove)
   - **Images**: Visualização e remoção de imagens Docker
   - **Networks**: Gerenciamento de redes personalizadas
   - **Volumes**: Gerenciamento de volumes ativos

3. **Funcionalidades principais:**
   - **Monitoramento**: Gráficos atualizados a cada segundo (local e remoto)
   - **Controle**: Ações em containers, imagens, networks e volumes
   - **SSH Remoto**: Conecte a servidores e gerencie Docker remotamente
   - **Toggle automático**: Sistema alterna entre local/remoto automaticamente
   - **Proteção**: Impede remoção de recursos em uso
   - **Feedback**: Mensagens de sucesso/erro com auto-dismiss
   - **Consistência**: Listas mantêm ordem alfabética

## 🏗️ Arquitetura

### Interface Modular
A aplicação utiliza uma arquitetura modular com componentes Slint separados:

- **`app.slint`** - Janela principal e navegação
- **`dashboard.slint`** - Dashboard com estatísticas e gráficos
- **`containers.slint`** - Lista e gerenciamento de containers
- **`container.slint`** - Componentes individuais de container
- **`images.slint`** - Gerenciamento de imagens Docker
- **`network.slint`** - Configuração de redes
- **`volumes.slint`** - Gerenciamento de volumes

### Backend Rust
- **`main.rs`** - Orquestração e estado da aplicação
- **`docker.rs`** - API Docker local e coleta de métricas
- **`docker_remote.rs`** - API Docker remota via SSH com funcionalidade completa
- **`docker_manager_switch.rs`** - Sistema de alternância entre local/remoto
- **`ssh.rs`** - Cliente SSH para conexões remotas
- **`ssh_persistence.rs`** - Persistência de configurações de servidores
- **`ssh_ui_integration.rs`** - Integração SSH com interface gráfica
- **`chart.rs`** - Renderização de gráficos em tempo real

### Sistema de Build
- **`build-deb.sh`** - Script de build versionado para pacotes .deb
- **`clean-builds.sh`** - Gerenciamento e limpeza de builds antigos
- **`Makefile`** - Automação completa do processo de build
- **`builds/`** - Diretório de saída para pacotes gerados

## 🔧 Tecnologias

- **[Rust](https://rust-lang.org/)** - Linguagem de programação
- **[Slint](https://slint.dev/)** - Framework de interface gráfica
- **[Bollard](https://github.com/fussybeaver/bollard)** - Client Docker para Rust
- **[SSH2](https://docs.rs/ssh2/)** - Cliente SSH para conexões remotas
- **[Plotters](https://github.com/plotters-rs/plotters)** - Biblioteca de gráficos
- **[Tokio](https://tokio.rs/)** - Runtime assíncrono
- **[Serde](https://serde.rs/)** - Serialização JSON para persistência

## 📊 Métricas monitoradas

### Local e Remoto via SSH:
- **CPU**: Porcentagem de uso em tempo real
- **Memória**: Uso e limite com porcentagem
- **Rede**: Bytes recebidos (RX) e transmitidos (TX)
- **I/O Disco**: Operações de leitura e escrita
- **Containers**: Total, rodando, parados e pausados
- **Imagens**: Quantidade total de imagens
- **Status de conexão**: Local ou remoto ativo

## 🎨 Interface

A aplicação possui:
- **Tema escuro** com cores modernas
- **Cards informativos** para estatísticas principais
- **Gráficos de linha** para dados temporais
- **Status visual** com cores indicativas
- **Layout responsivo** adaptável

### 🎯 Ícones disponíveis

A aplicação inclui ícones em múltiplos tamanhos para diferentes usos:

| Tamanho | Arquivo | Uso |
|---------|---------|-----|
| 32x32   | `assets/32x32.png` | Ícone pequeno |
| 48x48   | `assets/48x48.png` | Ícone médio |
| 64x64   | `assets/64x64.png` | Ícone padrão |
| 96x96   | `assets/96x96.png` | Ícone grande |
| 128x128 | `assets/128x128.png` | Ícone HD |
| -       | `assets/icon.png` | Ícone principal |
| -       | `assets/icon.ico` | Windows |
| -       | `assets/icon.icns` | macOS |

## 🐛 Solução de problemas

### Docker não conecta
```bash
# Verificar se Docker está rodando
sudo systemctl status docker

# Iniciar Docker se necessário
sudo systemctl start docker

# Adicionar usuário ao grupo docker
sudo usermod -aG docker $USER
```

### Erro de compilação
```bash
# Limpar cache do Cargo
cargo clean

# Atualizar dependências
cargo update

# Recompilar
cargo build
```

## 🤝 Contribuindo

1. Fork o projeto
2. Crie uma branch para sua feature (`git checkout -b feature/nova-feature`)
3. Commit suas mudanças (`git commit -am 'Adiciona nova feature'`)
4. Push para a branch (`git push origin feature/nova-feature`)
5. Abra um Pull Request

## 📝 Licença

Este projeto está licenciado sob a [MIT License](LICENSE).

## 🚀 Próximas funcionalidades

- [x] **Arquitetura modular** - Componentes Slint separados ✅
- [x] **Interface responsiva** - Layout otimizado ✅
- [x] **Gerenciamento remoto SSH** - Docker via SSH ✅
- [x] **Toggle automático** - Local/remoto baseado em conexão ✅
- [x] **Gerenciamento de containers** - Start/stop/restart via UI ✅
- [ ] **Visualização de logs** - Logs em tempo real
- [ ] **Métricas avançadas** - Histórico e exportação
- [ ] **Docker Compose** - Suporte a stacks
- [ ] **Temas personalizáveis** - Light/Dark mode
- [ ] **Configuração de alertas** - Notificações
- [ ] **Multi-host SSH** - Múltiplos servidores simultâneos

---

**Desenvolvido com ❤️ usando Rust e Slint**