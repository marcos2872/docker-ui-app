#!/bin/bash

# Script para build do Docker UI para openSUSE (RPM)
# Baseado no build-deb.sh existente

set -e

# Cores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Função para print colorido
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Verifica se está no diretório correto
if [ ! -f "Cargo.toml" ]; then
    print_error "Cargo.toml não encontrado. Execute este script no diretório raiz do projeto."
    exit 1
fi

# Extrai informações do Cargo.toml
APP_NAME=$(grep '^name = ' Cargo.toml | head -n1 | sed 's/name = "\(.*\)"/\1/')
APP_VERSION=$(grep '^version = ' Cargo.toml | head -n1 | sed 's/version = "\(.*\)"/\1/')
APP_DESCRIPTION=$(grep '^description = ' Cargo.toml | head -n1 | sed 's/description = "\(.*\)"/\1/')

# Use uma descrição padrão se não encontrar no Cargo.toml
if [ -z "$APP_DESCRIPTION" ]; then
    APP_DESCRIPTION="Interface gráfica moderna para gerenciamento Docker"
fi

# Normaliza o nome da aplicação para usar no pacote
if [ "$APP_NAME" = "teste-docker" ]; then
    APP_NAME="docker-ui"
fi

if [ -z "$APP_NAME" ] || [ -z "$APP_VERSION" ]; then
    print_error "Não foi possível extrair nome ou versão do Cargo.toml"
    exit 1
fi

print_status "Preparando build para $APP_NAME v$APP_VERSION"

# Verifica dependências necessárias
check_dependency() {
    if ! command -v $1 &> /dev/null; then
        print_error "$1 não está instalado"
        return 1
    fi
}

print_status "Verificando dependências..."

if ! check_dependency "cargo"; then
    print_error "Rust/Cargo não encontrado. Instale com: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

if ! check_dependency "rpmbuild"; then
    print_error "rpmbuild não encontrado. Instale com: sudo zypper install rpm-build"
    exit 1
fi

print_success "Dependências verificadas"

# Cria estrutura de diretórios para RPM
print_status "Criando estrutura de diretórios RPM..."
RPM_ROOT="$HOME/rpmbuild"
mkdir -p "$RPM_ROOT"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}

# Build do projeto
print_status "Fazendo build release do projeto..."
cargo build --release

# O nome do binário pode ser diferente do nome do pacote
BINARY_NAME=$(grep '^name = ' Cargo.toml | head -n1 | sed 's/name = "\(.*\)"/\1/')

if [ ! -f "target/release/$BINARY_NAME" ]; then
    print_error "Build falhou - binário não encontrado em target/release/$BINARY_NAME"
    exit 1
fi

print_success "Build concluído"

# Timestamp para versionamento
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RPM_VERSION="${APP_VERSION}_${TIMESTAMP}"
PACKAGE_NAME="${APP_NAME}-${RPM_VERSION}"

# Cria diretório de trabalho
WORK_DIR="/tmp/${PACKAGE_NAME}"
rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

print_status "Preparando arquivos do pacote..."

# Estrutura de diretórios do pacote
mkdir -p "$WORK_DIR/usr/bin"
mkdir -p "$WORK_DIR/usr/share/applications"
mkdir -p "$WORK_DIR/usr/share/icons/hicolor/32x32/apps"
mkdir -p "$WORK_DIR/usr/share/icons/hicolor/48x48/apps"
mkdir -p "$WORK_DIR/usr/share/icons/hicolor/64x64/apps"
mkdir -p "$WORK_DIR/usr/share/icons/hicolor/96x96/apps"
mkdir -p "$WORK_DIR/usr/share/icons/hicolor/128x128/apps"
mkdir -p "$WORK_DIR/usr/share/doc/$APP_NAME"

# Copia binário (pode ter nome diferente do pacote)
cp "target/release/$BINARY_NAME" "$WORK_DIR/usr/bin/$APP_NAME"
chmod 755 "$WORK_DIR/usr/bin/$APP_NAME"

# Copia ícones
if [ -f "assets/32x32.png" ]; then
    cp "assets/32x32.png" "$WORK_DIR/usr/share/icons/hicolor/32x32/apps/${APP_NAME}.png"
fi
if [ -f "assets/48x48.png" ]; then
    cp "assets/48x48.png" "$WORK_DIR/usr/share/icons/hicolor/48x48/apps/${APP_NAME}.png"
fi
if [ -f "assets/64x64.png" ]; then
    cp "assets/64x64.png" "$WORK_DIR/usr/share/icons/hicolor/64x64/apps/${APP_NAME}.png"
fi
if [ -f "assets/96x96.png" ]; then
    cp "assets/96x96.png" "$WORK_DIR/usr/share/icons/hicolor/96x96/apps/${APP_NAME}.png"
fi
if [ -f "assets/128x128.png" ]; then
    cp "assets/128x128.png" "$WORK_DIR/usr/share/icons/hicolor/128x128/apps/${APP_NAME}.png"
fi

# Cria arquivo .desktop
cat > "$WORK_DIR/usr/share/applications/${APP_NAME}.desktop" << EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=Docker UI
Comment=${APP_DESCRIPTION}
Exec=${APP_NAME}
Icon=${APP_NAME}
Categories=Development;System;
Terminal=false
StartupNotify=true
Keywords=docker;containers;monitoring;
EOF

# Copia documentação
if [ -f "README.md" ]; then
    cp "README.md" "$WORK_DIR/usr/share/doc/$APP_NAME/"
fi

# Cria arquivo spec para RPM
SPEC_FILE="$RPM_ROOT/SPECS/${APP_NAME}.spec"
cat > "$SPEC_FILE" << EOF
Name:           ${APP_NAME}
Version:        ${APP_VERSION}
Release:        $(date +%Y%m%d_%H%M%S)%{?dist}
Summary:        ${APP_DESCRIPTION}
License:        MIT
Group:          Applications/System
URL:            https://github.com/user/docker-ui-app
BuildArch:      x86_64
Requires:       docker

%description
Uma aplicação de monitoramento Docker construída com Rust e Slint, 
oferecendo uma interface gráfica moderna para visualizar estatísticas 
e gerenciar containers, imagens, networks e volumes.

Funcionalidades:
- Dashboard com estatísticas em tempo real
- Gerenciamento completo de containers
- Controle de imagens Docker
- Gerenciamento de networks personalizadas
- Controle de volumes persistentes

%prep
# Não precisamos de prep, usamos binários já compilados

%build
# Build já foi feito externamente

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
cp -a ${WORK_DIR}/* %{buildroot}/

%clean
rm -rf %{buildroot}

%files
%defattr(-,root,root,-)
/usr/bin/${APP_NAME}
/usr/share/applications/${APP_NAME}.desktop
/usr/share/icons/hicolor/32x32/apps/${APP_NAME}.png
/usr/share/icons/hicolor/48x48/apps/${APP_NAME}.png
/usr/share/icons/hicolor/64x64/apps/${APP_NAME}.png
/usr/share/icons/hicolor/96x96/apps/${APP_NAME}.png
/usr/share/icons/hicolor/128x128/apps/${APP_NAME}.png
%doc /usr/share/doc/${APP_NAME}/README.md

%post
/usr/bin/update-desktop-database &> /dev/null || :
/bin/touch --no-create /usr/share/icons/hicolor &>/dev/null || :

%postun
/usr/bin/update-desktop-database &> /dev/null || :
if [ \$1 -eq 0 ] ; then
    /bin/touch --no-create /usr/share/icons/hicolor &>/dev/null
    /usr/bin/gtk-update-icon-cache /usr/share/icons/hicolor &>/dev/null || :
fi

%posttrans
/usr/bin/gtk-update-icon-cache /usr/share/icons/hicolor &>/dev/null || :

%changelog
* $(date "+%a %b %d %Y") Builder <builder@localhost> - ${APP_VERSION}-$(date +%Y%m%d_%H%M%S)
- Build automático do Docker UI v${APP_VERSION}
- Interface gráfica moderna com Rust e Slint
- Monitoramento em tempo real de containers Docker
- Gerenciamento completo de recursos Docker

EOF

# Cria tarball dos sources
print_status "Criando tarball dos sources..."
cd /tmp
tar -czf "$RPM_ROOT/SOURCES/${PACKAGE_NAME}.tar.gz" "${PACKAGE_NAME##*/}"
cd - > /dev/null

# Build do RPM
print_status "Construindo pacote RPM..."
rpmbuild -ba "$SPEC_FILE"

# Cria diretório de builds se não existir
mkdir -p builds

# Move o RPM gerado
RPM_FILE=$(find "$RPM_ROOT/RPMS" -name "${APP_NAME}-*.rpm" -type f | head -n1)
if [ -n "$RPM_FILE" ]; then
    FINAL_RPM="builds/${APP_NAME}-${RPM_VERSION}.x86_64.rpm"
    cp "$RPM_FILE" "$FINAL_RPM"
    print_success "Pacote RPM criado: $FINAL_RPM"
    
    # Mostra informações do pacote
    print_status "Informações do pacote:"
    rpm -qip "$FINAL_RPM"
    
    print_status "Conteúdo do pacote:"
    rpm -qlp "$FINAL_RPM"
else
    print_error "Não foi possível encontrar o arquivo RPM gerado"
    exit 1
fi

# Limpeza
print_status "Limpando arquivos temporários..."
rm -rf "$WORK_DIR"
rm -rf "$RPM_ROOT/BUILD"/*
rm -rf "$RPM_ROOT/BUILDROOT"/*

print_success "Build concluído com sucesso!"
print_status "Para instalar: sudo rpm -ivh $FINAL_RPM"
print_status "Para desinstalar: sudo rpm -e $APP_NAME"
print_status "Para atualizar: sudo rpm -Uvh $FINAL_RPM"

# Informações adicionais
echo
print_status "Comandos úteis do openSUSE:"
echo "  - Instalar dependências: sudo zypper install docker"
echo "  - Verificar instalação: rpm -qa | grep $APP_NAME"
echo "  - Ver logs de instalação: rpm -q --changelog $APP_NAME"
echo "  - Buscar arquivos: rpm -ql $APP_NAME"