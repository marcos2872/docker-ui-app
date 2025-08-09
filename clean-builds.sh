#!/bin/bash

# Script para limpeza de builds antigos

set -e

# Cores
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

BUILDS_DIR="$(pwd)/builds"
APP_NAME="docker-ui"

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
}

# Função para listar builds
list_builds() {
    echo -e "${BLUE}📦 Builds encontrados:${NC}"
    
    local deb_found=false
    local rpm_found=false
    
    # Lista arquivos DEB
    if [ -d "$BUILDS_DIR" ] && [ -n "$(ls -A "$BUILDS_DIR"/*.deb 2>/dev/null)" ]; then
        echo -e "${YELLOW}   📦 Pacotes DEB:${NC}"
        ls -lh "$BUILDS_DIR"/*.deb 2>/dev/null | while read -r line; do
            echo "     $line"
        done
        deb_found=true
    fi
    
    # Lista arquivos RPM
    if [ -d "$BUILDS_DIR" ] && [ -n "$(ls -A "$BUILDS_DIR"/*.rpm 2>/dev/null)" ]; then
        echo -e "${YELLOW}   📦 Pacotes RPM:${NC}"
        ls -lh "$BUILDS_DIR"/*.rpm 2>/dev/null | while read -r line; do
            echo "     $line"
        done
        rpm_found=true
    fi
    
    if [ "$deb_found" = false ] && [ "$rpm_found" = false ]; then
        echo "   Nenhum build encontrado"
    else
        echo ""
        local total_size=$(du -sh "$BUILDS_DIR" 2>/dev/null | cut -f1)
        echo -e "${YELLOW}💾 Tamanho total: $total_size${NC}"
    fi
    echo ""
}

# Função para limpar builds antigos (manter apenas os N mais recentes)
clean_old_builds() {
    local keep=${1:-5}
    
    log "Mantendo apenas os $keep builds mais recentes de cada tipo..."
    
    if [ ! -d "$BUILDS_DIR" ]; then
        warning "Diretório builds não existe"
        return
    fi
    
    local total_removed=0
    
    # Limpa arquivos .deb antigos
    local deb_files=$(ls -t "$BUILDS_DIR"/${APP_NAME}_*.deb 2>/dev/null | tail -n +$((keep + 1)))
    if [ -n "$deb_files" ]; then
        echo -e "${YELLOW}   Limpando pacotes DEB antigos...${NC}"
        for file in $deb_files; do
            echo "   Removendo: $(basename "$file")"
            rm -f "$file"
            
            # Remove arquivo .info correspondente se existir
            local info_file="${file%.deb}_build.info"
            if [ -f "$info_file" ]; then
                rm -f "$info_file"
            fi
            
            ((total_removed++))
        done
    fi
    
    # Limpa arquivos .rpm antigos
    local rpm_files=$(ls -t "$BUILDS_DIR"/${APP_NAME}-*.rpm 2>/dev/null | tail -n +$((keep + 1)))
    if [ -n "$rpm_files" ]; then
        echo -e "${YELLOW}   Limpando pacotes RPM antigos...${NC}"
        for file in $rpm_files; do
            echo "   Removendo: $(basename "$file")"
            rm -f "$file"
            ((total_removed++))
        done
    fi
    
    if [ $total_removed -eq 0 ]; then
        success "Nenhum build antigo para remover"
    else
        success "$total_removed builds antigos removidos"
    fi
}

# Função para limpar tudo
clean_all() {
    log "Removendo todos os builds..."
    
    if [ -d "$BUILDS_DIR" ]; then
        local deb_count=$(ls "$BUILDS_DIR"/*.deb 2>/dev/null | wc -l)
        local rpm_count=$(ls "$BUILDS_DIR"/*.rpm 2>/dev/null | wc -l)
        local total_count=$((deb_count + rpm_count))
        
        rm -rf "$BUILDS_DIR"
        success "$total_count arquivos removidos ($deb_count DEB + $rpm_count RPM)"
    else
        warning "Diretório builds não existe"
    fi
}

# Função para limpar por versão
clean_version() {
    local version=$1
    
    if [ -z "$version" ]; then
        error "Versão não especificada"
        return 1
    fi
    
    log "Removendo builds da versão $version..."
    
    local removed_count=0
    
    # Remove arquivos DEB da versão
    local deb_pattern="${BUILDS_DIR}/${APP_NAME}_${version}-*"
    local deb_files=$(ls $deb_pattern 2>/dev/null || true)
    
    if [ -n "$deb_files" ]; then
        echo -e "${YELLOW}   Removendo pacotes DEB da versão $version...${NC}"
        for file in $deb_files; do
            echo "   Removendo: $(basename "$file")"
            rm -f "$file"
            ((removed_count++))
        done
    fi
    
    # Remove arquivos RPM da versão
    local rpm_pattern="${BUILDS_DIR}/${APP_NAME}-${version}-*"
    local rpm_files=$(ls $rpm_pattern 2>/dev/null || true)
    
    if [ -n "$rpm_files" ]; then
        echo -e "${YELLOW}   Removendo pacotes RPM da versão $version...${NC}"
        for file in $rpm_files; do
            echo "   Removendo: $(basename "$file")"
            rm -f "$file"
            ((removed_count++))
        done
    fi
    
    if [ $removed_count -eq 0 ]; then
        warning "Nenhum build encontrado para versão $version"
    else
        success "$removed_count arquivos removidos"
    fi
}

# Função para mostrar uso
show_usage() {
    echo -e "${BLUE}Docker UI Build Cleaner${NC}"
    echo ""
    echo "Uso: $0 [opção] [parâmetro]"
    echo ""
    echo "Opções:"
    echo "   list              Lista todos os builds"
    echo "   clean [N]         Remove builds antigos (mantém N mais recentes, padrão: 5)"
    echo "   clean-version V   Remove todos os builds da versão V"
    echo "   clean-all         Remove todos os builds"
    echo "   help              Mostra esta ajuda"
    echo ""
    echo "Exemplos:"
    echo "   $0 list"
    echo "   $0 clean 3"
    echo "   $0 clean-version 1.0.0"
    echo "   $0 clean-all"
}

# Função principal
main() {
    local action=${1:-list}
    
    case $action in
        "list")
            list_builds
            ;;
        "clean")
            local keep=${2:-5}
            list_builds
            clean_old_builds "$keep"
            echo ""
            list_builds
            ;;
        "clean-version")
            local version=$2
            list_builds
            clean_version "$version"
            echo ""
            list_builds
            ;;
        "clean-all")
            list_builds
            read -p "Tem certeza que deseja remover TODOS os builds? (y/N): " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                clean_all
            else
                warning "Operação cancelada"
            fi
            ;;
        "help"|"-h"|"--help")
            show_usage
            ;;
        *)
            error "Opção inválida: $action"
            show_usage
            exit 1
            ;;
    esac
}

# Executa se chamado diretamente
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi