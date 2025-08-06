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
  <img src="images/Captura de tela de 2025-08-06 11-24-16.png" alt="Dashboard Principal" width="600"/>
  <br>
  <em>Dashboard principal com estatísticas em tempo real</em>
</p>

<p align="center">
  <img src="images/Captura de tela de 2025-08-06 11-24-20.png" alt="Gráficos de Monitoramento" width="600"/>
  <br>
  <em>Gráficos de CPU e memória em tempo real</em>
</p>

### 🖼️ Galeria de Interface

<table>
  <tr>
    <td align="center">
      <img src="images/Captura de tela de 2025-08-06 11-24-16.png" width="300"/>
      <br><strong>Dashboard Overview</strong>
      <br><em>Cards de estatísticas e status</em>
    </td>
    <td align="center">
      <img src="images/Captura de tela de 2025-08-06 11-24-20.png" width="300"/>
      <br><strong>Monitoramento em Tempo Real</strong>
      <br><em>Gráficos de CPU, memória e rede</em>
    </td>
  </tr>
</table>

## ✨ Funcionalidades

- 📊 **Dashboard em tempo real** - Monitoramento de CPU, memória e rede
- 📈 **Gráficos interativos** - Visualização de dados históricos com plotters
- 🐳 **Status do Docker** - Verificação automática do estado do daemon
- 📋 **Informações detalhadas** - Containers, imagens, volumes e redes
- 🎨 **Interface moderna** - Design dark com tema responsivo

## 🚀 Pré-requisitos

- **Rust** 1.70+ 
- **Docker** instalado e rodando
- **Dependências do sistema** (Ubuntu/Debian):
  ```bash
  sudo apt update
  sudo apt install build-essential pkg-config libfontconfig1-dev
  ```

## 📦 Instalação

1. **Clone o repositório:**
   ```bash
   git clone <url-do-repositorio>
   cd teste-docker
   ```

2. **Compile e execute:**
   ```bash
   cargo run
   ```

## 🛠️ Desenvolvimento

### Modo watch (recompilação automática)
```bash
# Instalar cargo-watch
cargo install cargo-watch

# Executar em modo watch
cargo watch -x run
```

### Estrutura do projeto
```
├── src/
│   ├── main.rs          # Aplicação principal e gerenciamento de estado
│   ├── docker.rs        # API Docker e coleta de estatísticas
│   ├── chart.rs         # Renderização de gráficos
│   └── build.rs         # Script de compilação Slint
├── ui/
│   ├── app.slint        # Interface principal
│   └── containers.slint # Componentes de containers
└── Cargo.toml           # Dependências do projeto
```

## 🎯 Como usar

1. **Execute a aplicação:**
   ```bash
   cargo run
   ```

2. **Navegue pelas abas:**
   - **Docker UI**: Dashboard principal com estatísticas
   - **Containers**: Lista e gerenciamento de containers
   - **Images**: Visualização de imagens Docker
   - **Networks**: Configuração de redes
   - **Volumes**: Gerenciamento de volumes

3. **Monitoramento:**
   - Gráficos são atualizados a cada segundo
   - Status do Docker é verificado automaticamente
   - Dados históricos mantêm últimos 60 pontos

## 🔧 Tecnologias

- **[Rust](https://rust-lang.org/)** - Linguagem de programação
- **[Slint](https://slint.dev/)** - Framework de interface gráfica
- **[Bollard](https://github.com/fussybeaver/bollard)** - Client Docker para Rust
- **[Plotters](https://github.com/plotters-rs/plotters)** - Biblioteca de gráficos
- **[Tokio](https://tokio.rs/)** - Runtime assíncrono

## 📊 Métricas monitoradas

- **CPU**: Porcentagem de uso em tempo real
- **Memória**: Uso e limite com porcentagem
- **Rede**: Bytes recebidos (RX) e transmitidos (TX)
- **I/O Disco**: Operações de leitura e escrita
- **Containers**: Total, rodando, parados e pausados
- **Imagens**: Quantidade total de imagens

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

- [ ] Gerenciamento completo de containers (start/stop/restart)
- [ ] Visualização de logs em tempo real
- [ ] Exportação de métricas
- [ ] Configuração de alertas
- [ ] Suporte a Docker Compose
- [ ] Temas personalizáveis

---

**Desenvolvido com ❤️ usando Rust e Slint**