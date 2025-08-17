/// Módulo Remote para gerenciamento de múltiplos servidores Docker
/// 
/// Este módulo fornece funcionalidades para:
/// - Gerenciar múltiplos servidores Docker (locais e remotos via SSH)
/// - Adapter pattern para uniformizar interface Docker local/remoto
/// - Configuração e persistência de servidores remotos
/// - Monitoramento e estatísticas de servidores

pub mod manager;
pub mod docker_remote;

// Re-exports principais para facilitar o uso
pub use manager::{
    RemoteServerManager, 
    ServerInfo, 
    ServerType,
};

pub use docker_remote::{
    DockerRemoteAdapter,
};

/// Módulo de conveniência para importação simplificada
/// 
/// Exemplo de uso:
/// ```rust
/// use crate::remote::prelude::*;
/// 
/// let manager = RemoteServerManager::new();
/// let adapter = DockerRemoteFactory::create_remote_adapter(ssh_config);
/// ```
pub mod prelude {
    pub use super::manager::{
        RemoteServerManager, 
        ServerInfo, 
        ServerType,
    };
    
    pub use super::docker_remote::{
        DockerRemoteAdapter,
    };
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::ssh::{SshServerConfig, AuthMethod};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_full_integration() {
        // Testa integração completa entre manager e adapter
        let manager = RemoteServerManager::new();
        
        // Adiciona servidor local
        let local_id = manager.add_local_server(
            "Local Docker".to_string(),
            Some("Servidor Docker local".to_string())
        ).await.unwrap();
        
        // Adiciona servidor remoto
        let ssh_config = SshServerConfig::new_with_password(
            "remote_server".to_string(),
            "192.168.1.100".to_string(),
            22,
            "admin".to_string(),
            "secret".to_string(),
        );
        
        let remote_id = manager.add_remote_server(
            "Remote Docker".to_string(),
            ssh_config.clone(),
            Some("Servidor Docker remoto de produção".to_string())
        ).await.unwrap();
        
        // Verifica servidores adicionados
        let servers = manager.list_servers().await;
        assert_eq!(servers.len(), 2);
        
        // Cria adapter para servidor remoto
        let remote_adapter = DockerRemoteFactory::create_remote_adapter(ssh_config);
        assert_eq!(remote_adapter.get_server_name(), "remote_server");
        
        // Verifica se pode obter cliente SSH do manager
        let ssh_client = manager.get_ssh_client(&remote_id).await;
        assert!(ssh_client.is_some());
        
        // Testa conectividade
        let connectivity_results = manager.connect_all().await;
        assert_eq!(connectivity_results.len(), 2);
        
        // Obtém estatísticas
        let stats = manager.get_statistics().await;
        assert_eq!(stats["total_servers"], 2);
        assert_eq!(stats["local_servers"], 1);
        assert_eq!(stats["remote_servers"], 1);
        
        // Remove servidores
        assert!(manager.remove_server(&local_id).await.unwrap());
        assert!(manager.remove_server(&remote_id).await.unwrap());
        
        // Verifica remoção
        let servers_after = manager.list_servers().await;
        assert_eq!(servers_after.len(), 0);
    }

    #[tokio::test]
    async fn test_export_import_config() {
        let manager = RemoteServerManager::new();
        
        // Adiciona alguns servidores
        let ssh_config = SshServerConfig::new_with_private_key(
            "server1".to_string(),
            "host1.example.com".to_string(),
            22,
            "user1".to_string(),
            PathBuf::from("/home/user/.ssh/id_rsa"),
            None,
        );
        
        manager.add_remote_server(
            "Server 1".to_string(),
            ssh_config,
            Some("Primeiro servidor".to_string())
        ).await.unwrap();
        
        manager.add_local_server(
            "Local".to_string(),
            Some("Docker local".to_string())
        ).await.unwrap();
        
        // Exporta configuração
        let exported = manager.export_servers_config().await.unwrap();
        assert!(!exported.is_empty());
        
        // Limpa e importa
        let cleared_count = manager.clear_all_servers().await.unwrap();
        assert_eq!(cleared_count, 2);
        
        let imported_count = manager.import_servers_config(&exported).await.unwrap();
        assert_eq!(imported_count, 2);
        
        // Verifica se foi importado corretamente
        let servers = manager.list_servers().await;
        assert_eq!(servers.len(), 2);
    }

    #[tokio::test]
    async fn test_server_filtering() {
        let manager = RemoteServerManager::new();
        
        // Adiciona diferentes tipos de servidor
        let local_id = manager.add_local_server(
            "Local 1".to_string(),
            None
        ).await.unwrap();
        
        let ssh_config = SshServerConfig::new_with_password(
            "remote1".to_string(),
            "remote1.com".to_string(),
            22,
            "user".to_string(),
            "pass".to_string(),
        );
        
        let remote_id = manager.add_remote_server(
            "Remote 1".to_string(),
            ssh_config,
            None
        ).await.unwrap();
        
        // Conecta apenas o servidor local
        manager.connect_to_server(&local_id).await.unwrap();
        
        // Testa filtros
        let all_servers = manager.list_servers().await;
        let active_servers = manager.list_active_servers().await;
        let local_servers = manager.list_local_servers().await;
        let remote_servers = manager.list_remote_servers().await;
        
        assert_eq!(all_servers.len(), 2);
        assert_eq!(active_servers.len(), 1);
        assert_eq!(local_servers.len(), 1);
        assert_eq!(remote_servers.len(), 1);
        
        // Verifica se o filtro retornou o servidor correto
        assert_eq!(active_servers[0].id, local_id);
        assert_eq!(local_servers[0].id, local_id);
        assert_eq!(remote_servers[0].id, remote_id);
    }
}