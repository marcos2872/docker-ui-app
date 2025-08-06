#!/bin/bash

# Script de build versionado para Docker UI
# Gera pacotes .deb organizados em /builds

set -e

# Cores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configurações
APP_NAME="docker-ui"
APP_DISPLAY_NAME="Docker UI"
APP_DESCRIPTION="Aplicação de monitoramento Docker com interface gráfica moderna"
MAINTAINER="Developer <dev@example.com>"
ARCHITECTURE="amd64"
CATEGORY="utils"
HOMEPAGE="https://github.com/example/docker-ui"

# Diretórios
PROJECT_DIR="$(pwd)"
BUILDS_DIR="${PROJECT_DIR}/builds"
BUILD_DIR="${BUILDS_DIR}/build"
DEB_DIR="${BUILD_DIR}/${APP_NAME}"

# Função para logging
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

success() {
    echo -e "${GREEN}✅ $1${NC}"
}

warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

error() {
    echo -e "${RED}❌ $1${NC}"
    exit 1
}

# Função para obter versão
get_version() {
    # Tenta obter do Cargo.toml
    if [ -f "Cargo.toml" ]; then
        VERSION=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/' | head -1)
        if [ -n "$VERSION" ]; then
            echo "$VERSION"
            return
        fi
    fi
    
    # Tenta obter do Git
    if git rev-parse --git-dir > /dev/null 2>&1; then
        GIT_VERSION=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
        if [ -n "$GIT_VERSION" ]; then
            echo "${GIT_VERSION#v}" # Remove 'v' prefix se existir
            return
        fi
    fi
    
    # Versão padrão
    echo "1.0.0"
}

# Função para obter build number
get_build_number() {
    local version=$1
    local build_base="${BUILDS_DIR}/${APP_NAME}_${version}"
    local build_num=1
    
    # Encontra o próximo número de build
    while [ -f "${build_base}-${build_num}_${ARCHITECTURE}.deb" ]; do
        ((build_num++))
    done
    
    echo "$build_num"
}

# Função para limpar build anterior
clean_build() {
    log "Limpando builds anteriores..."
    if [ -d "$BUILD_DIR" ]; then
        rm -rf "$BUILD_DIR"
    fi
    success "Build limpo"
}

# Função para criar estrutura de diretórios
create_structure() {
    log "Criando estrutura de diretórios..."
    
    mkdir -p "$BUILDS_DIR"
    mkdir -p "$BUILD_DIR"
    mkdir -p "$DEB_DIR"
    
    # Estrutura padrão do .deb
    mkdir -p "$DEB_DIR/DEBIAN"
    mkdir -p "$DEB_DIR/usr/bin"
    mkdir -p "$DEB_DIR/usr/share/applications"
    mkdir -p "$DEB_DIR/usr/share/pixmaps"
    mkdir -p "$DEB_DIR/usr/share/doc/${APP_NAME}"
    mkdir -p "$DEB_DIR/usr/share/${APP_NAME}/assets"
    
    success "Estrutura criada"
}

# Função para compilar aplicação
build_app() {
    log "Compilando aplicação em modo release..."
    
    # Limpa cache anterior
    cargo clean
    
    # Compila em modo release
    if ! cargo build --release; then
        error "Falha na compilação"
    fi
    
    success "Aplicação compilada"
}

# Função para copiar binário
copy_binary() {
    log "Copiando binário..."
    
    local binary_src="target/release/teste-docker"
    local binary_dst="$DEB_DIR/usr/bin/$APP_NAME"
    
    if [ ! -f "$binary_src" ]; then
        error "Binário não encontrado: $binary_src"
    fi
    
    cp "$binary_src" "$binary_dst"
    chmod +x "$binary_dst"
    
    success "Binário copiado"
}

# Função para copiar assets
copy_assets() {
    log "Copiando assets..."
    
    # Copia ícones se existirem
    if [ -d "assets" ]; then
        cp -r assets/* "$DEB_DIR/usr/share/${APP_NAME}/assets/"
        
        # Copia ícone principal para pixmaps
        if [ -f "assets/icon.png" ]; then
            cp "assets/icon.png" "$DEB_DIR/usr/share/pixmaps/${APP_NAME}.png"
        fi
    fi
    
    # Copia documentação
    if [ -f "README.md" ]; then
        cp "README.md" "$DEB_DIR/usr/share/doc/${APP_NAME}/"
    fi
    
    success "Assets copiados"
}

# Função para criar arquivo .desktop
create_desktop_file() {
    log "Criando arquivo .desktop..."
    
    cat > "$DEB_DIR/usr/share/applications/${APP_NAME}.desktop" << EOF
[Desktop Entry]
Name=${APP_DISPLAY_NAME}
Comment=${APP_DESCRIPTION}
Exec=/usr/bin/${APP_NAME}
Icon=${APP_NAME}
Terminal=false
Type=Application
Categories=${CATEGORY};System;Monitor;
StartupNotify=true
EOF

    success "Arquivo .desktop criado"
}

# Função para criar arquivo de controle
create_control_file() {
    local version=$1
    local build_num=$2
    local installed_size=$(du -ks "$DEB_DIR" | cut -f1)
    
    log "Criando arquivo de controle..."
    
    cat > "$DEB_DIR/DEBIAN/control" << EOF
Package: ${APP_NAME}
Version: ${version}-${build_num}
Section: ${CATEGORY}
Priority: optional
Architecture: ${ARCHITECTURE}
Installed-Size: ${installed_size}
Depends: libc6, libfontconfig1
Maintainer: ${MAINTAINER}
Homepage: ${HOMEPAGE}
Description: ${APP_DESCRIPTION}
 Uma aplicação moderna de monitoramento Docker construída com Rust e Slint.
 Oferece interface gráfica intuitiva para visualizar estatísticas em tempo real,
 gerenciar containers, imagens, redes e volumes.
 .
 Funcionalidades principais:
 - Dashboard em tempo real
 - Gráficos interativos de CPU e memória
 - Gerenciamento de containers
 - Interface dark moderna
EOF

    success "Arquivo de controle criado"
}

# Função para criar scripts pós-instalação
create_postinst() {
    log "Criando script pós-instalação..."
    
    cat > "$DEB_DIR/DEBIAN/postinst" << 'EOF'
#!/bin/bash
set -e

# Atualiza cache de ícones
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q
fi

# Atualiza cache de MIME types
if command -v update-mime-database >/dev/null 2>&1; then
    update-mime-database /usr/share/mime
fi

echo "Docker UI instalado com sucesso!"
echo "Execute 'docker-ui' ou procure na lista de aplicações."

exit 0
EOF

    chmod +x "$DEB_DIR/DEBIAN/postinst"
    success "Script pós-instalação criado"
}

# Função para criar script de remoção
create_prerm() {
    log "Criando script de remoção..."
    
    cat > "$DEB_DIR/DEBIAN/prerm" << 'EOF'
#!/bin/bash
set -e

echo "Removendo Docker UI..."

exit 0
EOF

    chmod +x "$DEB_DIR/DEBIAN/prerm"
    success "Script de remoção criado"
}

# Função para definir permissões
set_permissions() {
    log "Definindo permissões..."
    
    # Define proprietário e permissões
    find "$DEB_DIR" -type d -exec chmod 755 {} \;
    find "$DEB_DIR" -type f -exec chmod 644 {} \;
    
    # Executáveis
    chmod +x "$DEB_DIR/usr/bin/${APP_NAME}"
    chmod +x "$DEB_DIR/DEBIAN/postinst"
    chmod +x "$DEB_DIR/DEBIAN/prerm"
    
    success "Permissões definidas"
}

# Função para construir pacote .deb
build_deb_package() {
    local version=$1
    local build_num=$2
    local output_file="${BUILDS_DIR}/${APP_NAME}_${version}-${build_num}_${ARCHITECTURE}.deb"
    
    log "Construindo pacote .deb..."
    
    # Constrói o pacote
    if ! dpkg-deb --build "$DEB_DIR" "$output_file"; then
        error "Falha ao construir pacote .deb"
    fi
    
    success "Pacote criado: $(basename "$output_file")"
    echo -e "${GREEN}📦 Pacote disponível em: $output_file${NC}"
    
    # Mostra informações do pacote
    echo -e "\n${BLUE}ℹ️  Informações do pacote:${NC}"
    dpkg-deb --info "$output_file"
    
    # Mostra tamanho do arquivo
    local file_size=$(du -h "$output_file" | cut -f1)
    echo -e "${GREEN}📊 Tamanho do arquivo: $file_size${NC}"
    
    return 0
}

# Função para validar pacote
validate_package() {
    local output_file="${BUILDS_DIR}/${APP_NAME}_${version}-${build_num}_${ARCHITECTURE}.deb"
    
    log "Validando pacote..."
    
    # Verifica se o arquivo foi criado
    if [ ! -f "$output_file" ]; then
        error "Arquivo .deb não foi criado"
    fi
    
    # Lista conteúdo do pacote
    echo -e "\n${BLUE}📋 Conteúdo do pacote:${NC}"
    dpkg-deb --contents "$output_file"
    
    success "Pacote validado"
}

# Função para criar arquivo de build info
create_build_info() {
    local version=$1
    local build_num=$2
    local build_info="${BUILDS_DIR}/${APP_NAME}_${version}-${build_num}_build.info"
    
    log "Criando informações de build..."
    
    cat > "$build_info" << EOF
# Build Information for ${APP_DISPLAY_NAME}
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
VERSION=${version}
BUILD_NUMBER=${build_num}
ARCHITECTURE=${ARCHITECTURE}
GIT_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
RUST_VERSION=$(rustc --version)
CARGO_VERSION=$(cargo --version)
HOST_OS=$(uname -s)
HOST_ARCH=$(uname -m)
EOF

    success "Build info criado: $(basename "$build_info")"
}

# Função principal
main() {
    echo -e "${BLUE}"
    echo "╔═══════════════════════════════════════╗"
    echo "║          Docker UI Builder            ║"
    echo "║      Gerador de pacotes .deb          ║"
    echo "╚═══════════════════════════════════════╝"
    echo -e "${NC}\n"
    
    # Verifica dependências
    log "Verificando dependências..."
    command -v cargo >/dev/null 2>&1 || error "Rust/Cargo não instalado"
    command -v dpkg-deb >/dev/null 2>&1 || error "dpkg-deb não instalado"
    success "Dependências OK"
    
    # Obtém versão e build number
    version=$(get_version)
    build_num=$(get_build_number "$version")
    
    echo -e "${YELLOW}📋 Informações do Build:${NC}"
    echo -e "   App: $APP_DISPLAY_NAME"
    echo -e "   Versão: $version"
    echo -e "   Build: $build_num"
    echo -e "   Arquitetura: $ARCHITECTURE"
    echo -e "   Output: ${BUILDS_DIR}/${APP_NAME}_${version}-${build_num}_${ARCHITECTURE}.deb"
    echo ""
    
    # Executa build
    clean_build
    create_structure
    build_app
    copy_binary
    copy_assets
    create_desktop_file
    create_control_file "$version" "$build_num"
    create_postinst
    create_prerm
    set_permissions
    build_deb_package "$version" "$build_num"
    validate_package
    create_build_info "$version" "$build_num"
    
    # Limpa arquivos temporários
    log "Limpando arquivos temporários..."
    rm -rf "$BUILD_DIR"
    success "Limpeza concluída"
    
    echo -e "\n${GREEN}🎉 Build concluído com sucesso!${NC}"
    echo -e "${BLUE}📦 Pacote disponível em: ${BUILDS_DIR}/${APP_NAME}_${version}-${build_num}_${ARCHITECTURE}.deb${NC}"
    echo -e "${YELLOW}💡 Para instalar: sudo dpkg -i ${BUILDS_DIR}/${APP_NAME}_${version}-${build_num}_${ARCHITECTURE}.deb${NC}"
}

# Executa se chamado diretamente
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi