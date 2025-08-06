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
    if [ -d "$BUILDS_DIR" ] && [ -n "$(ls -A "$BUILDS_DIR"/*.deb 2>/dev/null)" ]; then
        ls -lh "$BUILDS_DIR"/*.deb 2>/dev/null | while read -r line; do
            echo "   $line"
        done
        echo ""
        
        local total_size=$(du -sh "$BUILDS_DIR" 2>/dev/null | cut -f1)
        echo -e "${YELLOW}💾 Tamanho total: $total_size${NC}"
    else
        echo "   Nenhum build encontrado"
    fi
    echo ""
}

# Função para limpar builds antigos (manter apenas os N mais recentes)
clean_old_builds() {
    local keep=${1:-5}
    
    log "Mantendo apenas os $keep builds mais recentes..."
    
    if [ ! -d "$BUILDS_DIR" ]; then
        warning "Diretório builds não existe"
        return
    fi
    
    # Lista arquivos .deb por data (mais recentes primeiro)
    local files=$(ls -t "$BUILDS_DIR"/${APP_NAME}_*.deb 2>/dev/null | tail -n +$((keep + 1)))
    
    if [ -z "$files" ]; then
        success "Nenhum build antigo para remover"
        return
    fi
    
    local removed_count=0
    for file in $files; do
        echo "   Removendo: $(basename "$file")"
        rm -f "$file"
        
        # Remove arquivo .info correspondente se existir
        local info_file="${file%.deb}_build.info"
        if [ -f "$info_file" ]; then
            rm -f "$info_file"
        fi
        
        ((removed_count++))
    done
    
    success "$removed_count builds antigos removidos"
}

# Função para limpar tudo
clean_all() {
    log "Removendo todos os builds..."
    
    if [ -d "$BUILDS_DIR" ]; then
        local file_count=$(ls "$BUILDS_DIR"/*.deb 2>/dev/null | wc -l)
        rm -rf "$BUILDS_DIR"
        success "$file_count arquivos removidos"
    else
        warning "Diretório builds não existe"
    fi
}

# Função para limpar por versão
clean_version() {
    local version=$1
    
    if [ -z "$version" ]; then
        error "Versão não especificada"
    fi
    
    log "Removendo builds da versão $version..."
    
    local pattern="${BUILDS_DIR}/${APP_NAME}_${version}-*"
    local files=$(ls $pattern 2>/dev/null || true)
    
    if [ -z "$files" ]; then
        warning "Nenhum build encontrado para versão $version"
        return
    fi
    
    local removed_count=0
    for file in $files; do
        echo "   Removendo: $(basename "$file")"
        rm -f "$file"
        ((removed_count++))
    done
    
    success "$removed_count arquivos removidos"
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